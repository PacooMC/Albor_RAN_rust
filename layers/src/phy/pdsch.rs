/// PDSCH (Physical Downlink Shared Channel) implementation
/// Based on 3GPP TS 38.211, 38.212, and 38.214

use common::{CellConfig, ModulationScheme};
use tracing::{debug, info};
use std::sync::Arc;
use super::ldpc::PdschLdpcEncoder;
use super::dmrs::{calculate_pdsch_dmrs_cinit, generate_dmrs_sequence, DmrsSequenceGenerator, DmrsType, get_pdsch_dmrs_params, apply_cdm_weights};

/// PDSCH configuration
#[derive(Debug, Clone)]
pub struct PdschConfig {
    /// Transport block size in bytes
    pub tbs_bytes: usize,
    /// Modulation scheme
    pub modulation: ModulationScheme,
    /// Number of layers
    pub num_layers: u8,
    /// Redundancy version
    pub rv: u8,
    /// LDPC base graph (1 or 2)
    pub ldpc_base_graph: u8,
    /// New data indicator
    pub ndi: bool,
    /// HARQ process ID
    pub harq_id: u8,
    /// Frequency allocation (PRBs)
    pub prb_allocation: Vec<u16>,
    /// Time allocation (start symbol and length)
    pub start_symbol: u8,
    pub num_symbols: u8,
    /// DMRS configuration
    pub dmrs_type: u8,
    pub dmrs_additional_pos: u8,
    pub dmrs_config_type: u8,
    /// Scrambling ID
    pub n_id: u16,
    /// RNTI
    pub rnti: u16,
    /// Code block size (for rate matching)
    pub code_block_size: usize,
}

/// Transport block processing result
pub struct TransportBlockResult {
    /// Encoded and rate-matched bits
    pub encoded_bits: Vec<u8>,
    /// Number of code blocks
    pub num_code_blocks: usize,
    /// Code block size
    pub code_block_size: usize,
}

/// PDSCH processor
#[derive(Clone)]
pub struct PdschProcessor {
    cell_config: Arc<CellConfig>,
}

impl PdschProcessor {
    pub fn new(cell_config: Arc<CellConfig>) -> Self {
        Self { cell_config }
    }

    /// Process PDSCH for SIB1 transmission
    pub fn process_sib1_pdsch(
        &self,
        resource_grid: &mut super::resource_grid::ResourceGrid,
        sib1_payload: &[u8],
        config: &PdschConfig,
    ) {
        info!(
            "Processing PDSCH for SIB1: TBS={} bytes, MCS={:?}, RBs={}",
            config.tbs_bytes,
            config.modulation,
            config.prb_allocation.len()
        );

        // 1. Process transport block (CRC, segmentation, LDPC encoding)
        let tb_result = self.process_transport_block(sib1_payload, config);
        
        // 2. Scramble the encoded bits
        let scrambled_bits = self.scramble_bits(&tb_result.encoded_bits, config);
        
        // 3. Modulate the scrambled bits
        let modulated_symbols = self.modulate_bits(&scrambled_bits, config.modulation);
        
        // 4. Layer mapping (for single layer transmission)
        let layer_mapped = self.layer_mapping(&modulated_symbols, config.num_layers);
        
        // 5. Map to resource grid
        self.map_to_resource_grid(resource_grid, &layer_mapped, config);
        
        // 6. Generate DMRS for PDSCH
        self.generate_pdsch_dmrs(resource_grid, config);
    }

    /// Process transport block with CRC attachment, segmentation, and LDPC encoding
    fn process_transport_block(&self, payload: &[u8], config: &PdschConfig) -> TransportBlockResult {
        // 1. Attach transport block CRC (24 bits for TBS > 3824 bits)
        let tb_with_crc = if payload.len() * 8 > 3824 {
            self.attach_tb_crc(payload)
        } else {
            payload.to_vec()
        };
        
        // 2. Code block segmentation
        let (code_blocks, cb_size) = self.segment_transport_block(&tb_with_crc, config.ldpc_base_graph);
        
        // 3. Calculate total available bits for rate matching
        let total_res = self.calculate_available_res(config);
        let total_bits = total_res * self.get_bits_per_symbol(config.modulation);
        let bits_per_cb = total_bits / code_blocks.len();
        
        // 4. LDPC encoding and rate matching for each code block
        let mut all_encoded_bits = Vec::new();
        
        for (cb_idx, code_block) in code_blocks.iter().enumerate() {
            // Attach code block CRC if needed (when more than one CB)
            let cb_with_crc = if code_blocks.len() > 1 {
                self.attach_cb_crc(code_block)
            } else {
                code_block.clone()
            };
            
            // LDPC encoding with rate matching
            let ldpc_encoder = PdschLdpcEncoder::new();
            let rate_matched = ldpc_encoder.encode(
                &cb_with_crc,
                bits_per_cb,
                config.rv
            );
            
            all_encoded_bits.extend(rate_matched);
        }
        
        debug!(
            "Transport block processed: {} code blocks, {} total encoded bits",
            code_blocks.len(),
            all_encoded_bits.len()
        );
        
        TransportBlockResult {
            encoded_bits: all_encoded_bits,
            num_code_blocks: code_blocks.len(),
            code_block_size: cb_size,
        }
    }

    /// Attach 24-bit CRC to transport block
    fn attach_tb_crc(&self, payload: &[u8]) -> Vec<u8> {
        let mut tb_with_crc = payload.to_vec();
        
        // Convert bytes to bits for CRC calculation
        let mut bits = Vec::new();
        for byte in payload {
            for i in (0..8).rev() {
                bits.push((byte >> i) & 1);
            }
        }
        
        // Calculate CRC24A
        let crc = self.calculate_crc24a(&bits);
        
        // Append CRC bytes
        tb_with_crc.push((crc >> 16) as u8);
        tb_with_crc.push((crc >> 8) as u8);
        tb_with_crc.push(crc as u8);
        
        tb_with_crc
    }

    /// Segment transport block into code blocks
    fn segment_transport_block(&self, tb: &[u8], base_graph: u8) -> (Vec<Vec<u8>>, usize) {
        let tb_size_bits = tb.len() * 8;
        
        // Determine maximum code block size based on base graph
        let max_cb_size = if base_graph == 1 { 8448 } else { 3840 };
        
        // Calculate number of code blocks
        let num_cb = if tb_size_bits <= max_cb_size {
            1
        } else {
            ((tb_size_bits as f32) / (max_cb_size as f32 - 24.0)).ceil() as usize
        };
        
        // Calculate actual code block size
        let cb_size_bits = if num_cb == 1 {
            tb_size_bits
        } else {
            let total_bits = tb_size_bits + 24 * num_cb; // Account for CB CRCs
            (total_bits + num_cb - 1) / num_cb // Ceiling division
        };
        
        let cb_size_bytes = (cb_size_bits + 7) / 8;
        
        // Segment the transport block
        let mut code_blocks = Vec::new();
        for i in 0..num_cb {
            let start = i * cb_size_bytes;
            let end = ((i + 1) * cb_size_bytes).min(tb.len());
            
            let mut cb = tb[start..end].to_vec();
            
            // Pad with zeros if necessary
            while cb.len() < cb_size_bytes {
                cb.push(0);
            }
            
            code_blocks.push(cb);
        }
        
        (code_blocks, cb_size_bits)
    }

    /// Attach 24-bit CRC to code block
    fn attach_cb_crc(&self, code_block: &[u8]) -> Vec<u8> {
        let mut cb_with_crc = code_block.to_vec();
        
        // Convert to bits for CRC
        let mut bits = Vec::new();
        for byte in code_block {
            for i in (0..8).rev() {
                bits.push((byte >> i) & 1);
            }
        }
        
        // Calculate CRC24B
        let crc = self.calculate_crc24b(&bits);
        
        // Append CRC
        cb_with_crc.push((crc >> 16) as u8);
        cb_with_crc.push((crc >> 8) as u8);
        cb_with_crc.push(crc as u8);
        
        cb_with_crc
    }


    /// Scramble bits with PDSCH scrambling sequence
    fn scramble_bits(&self, bits: &[u8], config: &PdschConfig) -> Vec<u8> {
        // Initialize scrambling sequence
        let c_init = self.calculate_scrambling_cinit(config);
        let mut generator = DmrsSequenceGenerator::new(c_init);
        
        // Convert bytes to bits for proper scrambling
        let mut bit_vec = Vec::new();
        for byte in bits {
            for i in (0..8).rev() {
                bit_vec.push((byte >> i) & 1);
            }
        }
        
        // Scramble each bit
        let mut scrambled_bits = Vec::new();
        for &bit in &bit_vec {
            let scrambling_bit = generator.next_bit();
            scrambled_bits.push(bit ^ scrambling_bit);
        }
        
        // Pack bits back to bytes
        let mut scrambled_bytes = Vec::new();
        for chunk in scrambled_bits.chunks(8) {
            let mut byte = 0u8;
            for (i, &bit) in chunk.iter().enumerate() {
                byte |= bit << (7 - i);
            }
            scrambled_bytes.push(byte);
        }
        
        scrambled_bytes
    }

    /// Modulate bits to complex symbols
    fn modulate_bits(&self, bits: &[u8], modulation: ModulationScheme) -> Vec<num_complex::Complex32> {
        let mut symbols = Vec::new();
        let bits_per_symbol = self.get_bits_per_symbol(modulation);
        
        let mut bit_buffer = 0u8;
        let mut bits_in_buffer = 0;
        
        for byte in bits {
            for bit_pos in (0..8).rev() {
                bit_buffer = (bit_buffer << 1) | ((byte >> bit_pos) & 1);
                bits_in_buffer += 1;
                
                if bits_in_buffer == bits_per_symbol {
                    let symbol = match modulation {
                        ModulationScheme::Qpsk => self.modulate_qpsk(bit_buffer),
                        ModulationScheme::Qam16 => self.modulate_16qam(bit_buffer),
                        ModulationScheme::Qam64 => self.modulate_64qam(bit_buffer),
                        ModulationScheme::Qam256 => self.modulate_256qam(bit_buffer),
                    };
                    symbols.push(symbol);
                    bit_buffer = 0;
                    bits_in_buffer = 0;
                }
            }
        }
        
        symbols
    }

    /// Layer mapping for single layer
    fn layer_mapping(&self, symbols: &[num_complex::Complex32], _num_layers: u8) -> Vec<num_complex::Complex32> {
        // For single layer, just return the symbols as-is
        symbols.to_vec()
    }

    /// Map symbols to resource grid
    fn map_to_resource_grid(
        &self,
        resource_grid: &mut super::resource_grid::ResourceGrid,
        symbols: &[num_complex::Complex32],
        config: &PdschConfig,
    ) {
        let mut symbol_idx = 0;
        
        for ofdm_symbol in config.start_symbol..config.start_symbol + config.num_symbols {
            // Skip DMRS symbols
            if self.is_dmrs_symbol(ofdm_symbol, config) {
                continue;
            }
            
            for prb in &config.prb_allocation {
                for subcarrier in 0..12 {
                    let re_idx = *prb as usize * 12 + subcarrier;
                    
                    if symbol_idx < symbols.len() {
                        let _ = resource_grid.map_re(re_idx as u16, ofdm_symbol, symbols[symbol_idx]);
                        symbol_idx += 1;
                    }
                }
            }
        }
        
        info!("Mapped {} PDSCH symbols to resource grid", symbol_idx);
    }

    /// Generate DMRS for PDSCH
    fn generate_pdsch_dmrs(
        &self,
        resource_grid: &mut super::resource_grid::ResourceGrid,
        config: &PdschConfig,
    ) {
        // DMRS configuration
        const DMRS_AMPLITUDE: f32 = 0.7071067811865476; // 1/sqrt(2) for QPSK
        let dmrs_type = DmrsType::Type1; // Type 1 configuration
        let dmrs_port = 0; // Single port for now
        let n_scid = false; // Scrambling ID 0
        
        // Get DMRS parameters for Type 1, port 0
        let (dmrs_positions, dmrs_weights) = get_pdsch_dmrs_params(dmrs_type, dmrs_port);
        
        // Process each DMRS symbol
        for ofdm_symbol in config.start_symbol..config.start_symbol + config.num_symbols {
            if !self.is_dmrs_symbol(ofdm_symbol, config) {
                continue;
            }
            
            // Calculate DMRS initialization value for this symbol
            let slot = 0; // TODO: Get actual slot number from context
            let c_init = calculate_pdsch_dmrs_cinit(slot, ofdm_symbol, config.n_id, n_scid);
            
            // Create DMRS sequence generator
            let mut generator = DmrsSequenceGenerator::new(c_init);
            
            // Create RB mask for PRB allocation
            let mut rb_mask = vec![false; 275]; // Max RBs
            for &prb in &config.prb_allocation {
                if (prb as usize) < rb_mask.len() {
                    rb_mask[prb as usize] = true;
                }
            }
            
            // Generate base DMRS sequence for allocated RBs
            let base_sequence = generate_dmrs_sequence(
                &rb_mask,
                0, // Reference point (start of bandwidth)
                dmrs_type.nof_dmrs_per_rb(),
                &mut generator,
                DMRS_AMPLITUDE,
            );
            
            // Apply CDM weights if not port 0
            let dmrs_sequence = if dmrs_port == 0 {
                base_sequence
            } else {
                // Calculate l_prime (0 for first DMRS symbol, 1 for additional)
                let l_prime = if ofdm_symbol == config.start_symbol { 0 } else { 1 };
                apply_cdm_weights(&base_sequence, &dmrs_weights, l_prime)
            };
            
            // Map DMRS to resource grid
            let mut dmrs_idx = 0;
            for &prb in &config.prb_allocation {
                if dmrs_idx + dmrs_positions.len() <= dmrs_sequence.len() {
                    // Map DMRS to positions specified for this port
                    for (pos_idx, &k) in dmrs_positions.iter().enumerate() {
                        let _ = resource_grid.map_re(
                            prb * 12 + k as u16,
                            ofdm_symbol,
                            dmrs_sequence[dmrs_idx + pos_idx],
                        );
                    }
                    dmrs_idx += dmrs_positions.len();
                }
            }
        }
        
        debug!("Generated PDSCH DMRS (Type 1, port {}) for {} PRBs", dmrs_port, config.prb_allocation.len());
    }

    /// Helper functions
    fn calculate_available_res(&self, config: &PdschConfig) -> usize {
        let res_per_prb_per_symbol = 12; // All subcarriers for data
        let mut total_res = 0;
        
        for symbol in config.start_symbol..config.start_symbol + config.num_symbols {
            if !self.is_dmrs_symbol(symbol, config) {
                total_res += config.prb_allocation.len() * res_per_prb_per_symbol;
            } else {
                // DMRS symbol has half REs for data (Type A)
                total_res += config.prb_allocation.len() * res_per_prb_per_symbol / 2;
            }
        }
        
        total_res
    }

    fn is_dmrs_symbol(&self, symbol: u8, config: &PdschConfig) -> bool {
        // For SIB1, typically DMRS is in the first symbol of PDSCH
        symbol == config.start_symbol
    }

    fn get_bits_per_symbol(&self, modulation: ModulationScheme) -> usize {
        match modulation {
            ModulationScheme::Qpsk => 2,
            ModulationScheme::Qam16 => 4,
            ModulationScheme::Qam64 => 6,
            ModulationScheme::Qam256 => 8,
        }
    }

    fn calculate_crc24a(&self, bits: &[u8]) -> u32 {
        // CRC24A polynomial: x^24 + x^23 + x^18 + x^17 + x^14 + x^11 + x^10 + x^7 + x^6 + x^5 + x^4 + x^3 + x + 1
        let poly = 0x1864CFB;
        self.calculate_crc24(bits, poly)
    }

    fn calculate_crc24b(&self, bits: &[u8]) -> u32 {
        // CRC24B polynomial: x^24 + x^23 + x^6 + x^5 + x + 1
        let poly = 0x1800063;
        self.calculate_crc24(bits, poly)
    }

    fn calculate_crc24(&self, bits: &[u8], poly: u32) -> u32 {
        // Proper bit-by-bit CRC calculation as per srsRAN
        let mut remainder = 0u64;
        let order = 24;
        let highbit = 1u64 << order;
        
        // Process each bit
        for &bit in bits {
            remainder = (remainder << 1) | (bit as u64);
            
            if (remainder & highbit) != 0 {
                remainder ^= poly as u64;
            }
        }
        
        // Process remaining bits
        for _ in 0..order {
            remainder = remainder << 1;
            
            if (remainder & highbit) != 0 {
                remainder ^= poly as u64;
            }
        }
        
        (remainder & (highbit - 1)) as u32
    }

    fn calculate_scrambling_cinit(&self, config: &PdschConfig) -> u32 {
        // c_init = rnti * 2^15 + q * 2^14 + n_ID
        let q = 0; // Codeword index
        let c_init = (config.rnti as u32) * (1 << 15) + (q << 14) + config.n_id as u32;
        c_init & 0x7FFFFFFF
    }


    /// Modulation functions
    fn modulate_qpsk(&self, bits: u8) -> num_complex::Complex32 {
        let b0 = bits & 1;
        let b1 = (bits >> 1) & 1;
        
        let scale = 1.0 / std::f32::consts::SQRT_2;
        num_complex::Complex32::new(
            scale * (1.0 - 2.0 * b0 as f32),
            scale * (1.0 - 2.0 * b1 as f32),
        )
    }

    fn modulate_16qam(&self, bits: u8) -> num_complex::Complex32 {
        let b0 = bits & 1;
        let b1 = (bits >> 1) & 1;
        let b2 = (bits >> 2) & 1;
        let b3 = (bits >> 3) & 1;
        
        let scale = 1.0 / 10.0_f32.sqrt();
        let i = (1.0 - 2.0 * b0 as f32) * (2.0 - (1.0 - 2.0 * b2 as f32));
        let q = (1.0 - 2.0 * b1 as f32) * (2.0 - (1.0 - 2.0 * b3 as f32));
        
        num_complex::Complex32::new(scale * i, scale * q)
    }

    fn modulate_64qam(&self, bits: u8) -> num_complex::Complex32 {
        // Simplified 64QAM - proper implementation would use full constellation
        let scale = 1.0 / 42.0_f32.sqrt();
        let i_bits = bits & 0x7;
        let q_bits = (bits >> 3) & 0x7;
        
        let i = [-7.0, -5.0, -3.0, -1.0, 1.0, 3.0, 5.0, 7.0][i_bits as usize];
        let q = [-7.0, -5.0, -3.0, -1.0, 1.0, 3.0, 5.0, 7.0][q_bits as usize];
        
        num_complex::Complex32::new(scale * i, scale * q)
    }

    fn modulate_256qam(&self, bits: u8) -> num_complex::Complex32 {
        // Simplified 256QAM - proper implementation would use full constellation
        let scale = 1.0 / 170.0_f32.sqrt();
        let i_bits = bits & 0xF;
        let q_bits = (bits >> 4) & 0xF;
        
        let i = -15.0 + 2.0 * i_bits as f32;
        let q = -15.0 + 2.0 * q_bits as f32;
        
        num_complex::Complex32::new(scale * i, scale * q)
    }
}