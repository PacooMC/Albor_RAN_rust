/// PDCCH (Physical Downlink Control Channel) implementation
/// Based on 3GPP TS 38.211, 38.212, and 38.213

use common::{CellConfig, CorsetConfig};
use tracing::{debug, info};
use std::sync::Arc;
use num_complex::Complex32;
use super::polar::PdcchPolarEncoder;
use super::dmrs::{calculate_pdcch_dmrs_cinit, generate_dmrs_sequence, DmrsSequenceGenerator};

/// DCI Format 1_0 for SI-RNTI (System Information)
#[derive(Debug, Clone)]
pub struct DciFormat10SiRnti {
    /// Frequency domain resource assignment
    pub frequency_resource: u16,
    /// Time domain resource assignment (0-3 for SI-RNTI)
    pub time_resource: u8,
    /// VRB-to-PRB mapping (0: non-interleaved, 1: interleaved)
    pub vrb_to_prb_mapping: u8,
    /// Modulation and coding scheme (0-31)
    pub modulation_coding_scheme: u8,
    /// Redundancy version (0-3)
    pub redundancy_version: u8,
    /// System information indicator (0: SIB1, 1: SI message)
    pub system_information_indicator: u8,
}

/// PDCCH encoder configuration
pub struct PdcchEncoderConfig {
    /// Total number of encoded bits (E)
    pub encoded_bits: usize,
    /// RNTI value
    pub rnti: u16,
}

/// PDCCH modulator configuration
pub struct PdcchModulatorConfig {
    /// RB mask indicating which RBs are used
    pub rb_mask: Vec<bool>,
    /// Starting symbol index
    pub start_symbol_index: u8,
    /// Duration in symbols
    pub duration: u8,
    /// Scrambling ID for data
    pub n_id_data: u16,
    /// Scrambling ID for DMRS
    pub n_id_dmrs: u16,
    /// RNTI for scrambling
    pub n_rnti: u16,
    /// Power scaling factor
    pub scaling: f32,
}

/// PDCCH processor
#[derive(Clone)]
pub struct PdcchProcessor {
    cell_config: Arc<CellConfig>,
}

impl PdcchProcessor {
    pub fn new(cell_config: Arc<CellConfig>) -> Self {
        Self { cell_config }
    }

    /// Process PDCCH for SIB1 scheduling
    pub fn process_sib1_pdcch(
        &self,
        resource_grid: &mut super::resource_grid::ResourceGrid,
        coreset: &CorsetConfig,
        dci: &DciFormat10SiRnti,
        aggregation_level: u8,
        cce_index: u16,
    ) {
        info!(
            "Processing PDCCH for SIB1: AL={}, CCE={}",
            aggregation_level, cce_index
        );

        // 1. Encode DCI payload
        let dci_bits = self.encode_dci_format_1_0_si_rnti(dci, coreset);
        
        // 2. Add CRC and scramble with SI-RNTI
        let crc_attached = self.attach_crc_and_scramble(&dci_bits, 0xFFFF); // SI-RNTI = 0xFFFF
        
        // 3. Perform Polar encoding
        let encoded_bits = self.polar_encode(&crc_attached, aggregation_level);
        
        // 4. Scramble the encoded bits
        let scrambled_bits = self.scramble_data(&encoded_bits, 0xFFFF, 0); // Using 0 for CORESET0
        
        // 5. Map to CORESET resources with modulation
        self.map_to_coreset(resource_grid, coreset, &scrambled_bits, aggregation_level, cce_index);
        
        // 6. Generate DMRS for PDCCH
        self.generate_pdcch_dmrs(resource_grid, coreset, aggregation_level, cce_index);
    }

    /// Encode DCI Format 1_0 for SI-RNTI
    fn encode_dci_format_1_0_si_rnti(&self, dci: &DciFormat10SiRnti, coreset: &CorsetConfig) -> Vec<u8> {
        let mut bits = Vec::new();
        
        // Frequency domain resource assignment (depends on CORESET bandwidth)
        let freq_bits = self.calculate_frequency_domain_bits(coreset);
        self.append_bits(&mut bits, dci.frequency_resource as u32, freq_bits);
        
        // Time domain resource assignment (2 bits for SI-RNTI)
        self.append_bits(&mut bits, dci.time_resource as u32, 2);
        
        // VRB-to-PRB mapping (1 bit)
        self.append_bits(&mut bits, dci.vrb_to_prb_mapping as u32, 1);
        
        // Modulation and coding scheme (5 bits)
        self.append_bits(&mut bits, dci.modulation_coding_scheme as u32, 5);
        
        // Redundancy version (2 bits)
        self.append_bits(&mut bits, dci.redundancy_version as u32, 2);
        
        // System information indicator (1 bit)
        self.append_bits(&mut bits, dci.system_information_indicator as u32, 1);
        
        // Reserved bits (depends on format size alignment)
        let total_bits = self.calculate_dci_size(coreset);
        while bits.len() < total_bits {
            bits.push(0);
        }
        
        debug!("Encoded DCI Format 1_0 SI-RNTI: {} bits", bits.len());
        bits
    }

    /// Attach CRC and scramble with RNTI
    fn attach_crc_and_scramble(&self, dci_bits: &[u8], rnti: u16) -> Vec<u8> {
        // Add 24 leading 1s for CRC calculation (as per srsRAN implementation)
        let mut padded_bits = vec![1u8; 24];
        padded_bits.extend_from_slice(dci_bits);
        
        // Calculate 24-bit CRC using CRC24C polynomial over padded bits
        let crc = self.calculate_crc24c(&padded_bits);
        
        // Combine DCI bits and CRC (without the leading 1s)
        let mut combined = dci_bits.to_vec();
        
        // Append CRC bits
        for i in 0..24 {
            combined.push(((crc >> (23 - i)) & 1) as u8);
        }
        
        // Scramble last 16 bits of CRC with RNTI
        let rnti_bits = self.u16_to_bits(rnti);
        for i in 0..16 {
            let idx = combined.len() - 16 + i;
            combined[idx] ^= rnti_bits[i];
        }
        
        combined
    }

    /// Perform Polar encoding
    fn polar_encode(&self, input: &[u8], aggregation_level: u8) -> Vec<u8> {
        // Use proper Polar encoder
        let polar_encoder = PdcchPolarEncoder::new();
        let encoded = polar_encoder.encode(input, aggregation_level);
        
        debug!(
            "Polar encoded: {} input bits to {} output bits",
            input.len(),
            encoded.len()
        );
        
        encoded
    }

    /// Map encoded bits to CORESET resources
    fn map_to_coreset(
        &self,
        resource_grid: &mut super::resource_grid::ResourceGrid,
        coreset: &CorsetConfig,
        encoded_bits: &[u8],
        aggregation_level: u8,
        cce_index: u16,
    ) {
        // Calculate PRB indices for this PDCCH
        let prb_indices = self.calculate_prb_indices(coreset, aggregation_level, cce_index);
        
        // QPSK modulation
        let mut bit_idx = 0;
        for prb in &prb_indices {
            for symbol in coreset.start_symbol..coreset.start_symbol + coreset.duration {
                // Skip DMRS positions (every 4th subcarrier)
                for subcarrier in 0..12 {
                    if subcarrier % 4 == 1 {
                        continue; // DMRS position
                    }
                    
                    if bit_idx + 1 < encoded_bits.len() {
                        // QPSK modulation
                        let i = encoded_bits[bit_idx] as f32;
                        let q = encoded_bits[bit_idx + 1] as f32;
                        let scale = 1.0 / std::f32::consts::SQRT_2;
                        let symbol_value = scale * Complex32::new(
                            1.0 - 2.0 * i,
                            1.0 - 2.0 * q,
                        );
                        
                        let _ = resource_grid.map_re(
                            *prb * 12 + subcarrier as u16,
                            symbol,
                            symbol_value,
                        );
                        
                        bit_idx += 2;
                    }
                }
            }
        }
        
        info!(
            "Mapped PDCCH to {} PRBs, {} bits",
            prb_indices.len(),
            bit_idx
        );
    }

    /// Generate DMRS for PDCCH
    fn generate_pdcch_dmrs(
        &self,
        resource_grid: &mut super::resource_grid::ResourceGrid,
        coreset: &CorsetConfig,
        aggregation_level: u8,
        cce_index: u16,
    ) {
        let prb_indices = self.calculate_prb_indices(coreset, aggregation_level, cce_index);
        
        // Constants for PDCCH DMRS
        const DMRS_PER_RB: usize = 3; // DMRS on subcarriers 1, 5, 9
        const DMRS_AMPLITUDE: f32 = 0.7071067811865476; // 1/sqrt(2) for QPSK
        
        // Process each symbol in CORESET
        for symbol in coreset.start_symbol..coreset.start_symbol + coreset.duration {
            // Calculate DMRS initialization value for this symbol
            let slot = 0; // TODO: Get actual slot number from context
            let c_init = calculate_pdcch_dmrs_cinit(slot, symbol, self.cell_config.pci);
            
            // Create DMRS sequence generator
            let mut generator = DmrsSequenceGenerator::new(c_init);
            
            // Create RB mask for PRB allocation
            let mut rb_mask = vec![false; 275]; // Max RBs
            for &prb in &prb_indices {
                if (prb as usize) < rb_mask.len() {
                    rb_mask[prb as usize] = true;
                }
            }
            
            // Generate DMRS sequence for allocated RBs
            let dmrs_sequence = generate_dmrs_sequence(
                &rb_mask,
                0, // Reference point (start of bandwidth)
                DMRS_PER_RB,
                &mut generator,
                DMRS_AMPLITUDE,
            );
            
            // Map DMRS to resource grid
            let mut dmrs_idx = 0;
            for &prb in &prb_indices {
                if dmrs_idx + DMRS_PER_RB <= dmrs_sequence.len() {
                    // Map DMRS on subcarriers 1, 5, 9
                    let _ = resource_grid.map_re(
                        prb * 12 + 1,
                        symbol,
                        dmrs_sequence[dmrs_idx],
                    );
                    let _ = resource_grid.map_re(
                        prb * 12 + 5,
                        symbol,
                        dmrs_sequence[dmrs_idx + 1],
                    );
                    let _ = resource_grid.map_re(
                        prb * 12 + 9,
                        symbol,
                        dmrs_sequence[dmrs_idx + 2],
                    );
                    dmrs_idx += DMRS_PER_RB;
                }
            }
        }
        
        debug!("Generated PDCCH DMRS for {} PRBs", prb_indices.len());
    }

    /// Calculate PRB indices for CCE mapping
    fn calculate_prb_indices(&self, coreset: &CorsetConfig, aggregation_level: u8, cce_index: u16) -> Vec<u16> {
        let mut prb_indices = Vec::new();
        
        // For CORESET0 and non-interleaved mapping
        let regs_per_cce = 6;
        let cces = aggregation_level as u16;
        
        for cce_offset in 0..cces {
            let current_cce = cce_index + cce_offset;
            for reg_offset in 0..regs_per_cce {
                let reg_index = current_cce * regs_per_cce + reg_offset;
                let prb = coreset.frequency_domain_resources[0] + reg_index / coreset.duration as u16;
                if !prb_indices.contains(&prb) {
                    prb_indices.push(prb);
                }
            }
        }
        
        prb_indices.sort();
        prb_indices
    }

    /// Helper functions
    fn calculate_frequency_domain_bits(&self, coreset: &CorsetConfig) -> u8 {
        // Calculate based on CORESET bandwidth
        let n_rb = coreset.frequency_domain_resources.len() as u32;
        ((n_rb * (n_rb + 1) / 2) as f32).log2().ceil() as u8
    }

    fn calculate_dci_size(&self, _coreset: &CorsetConfig) -> usize {
        // DCI format 1_0 size calculation
        // For SI-RNTI, typical size is around 28-44 bits depending on bandwidth
        41 // Placeholder - should be calculated based on BWP size
    }

    fn append_bits(&self, bits: &mut Vec<u8>, value: u32, num_bits: u8) {
        for i in (0..num_bits).rev() {
            bits.push(((value >> i) & 1) as u8);
        }
    }

    fn calculate_crc24c(&self, data: &[u8]) -> u32 {
        // CRC24C polynomial: 0x1B2B117 (from srsRAN)
        let poly = 0x1B2B117;
        let mut remainder = 0u64;
        let order = 24;
        let highbit = 1u64 << order;
        
        // Process each bit
        for &bit in data {
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

    fn u16_to_bits(&self, value: u16) -> Vec<u8> {
        let mut bits = Vec::with_capacity(16);
        for i in (0..16).rev() {
            bits.push(((value >> i) & 1) as u8);
        }
        bits
    }

    
    /// Scramble data bits with pseudo-random sequence
    fn scramble_data(&self, data: &[u8], rnti: u16, _coreset_id: u8) -> Vec<u8> {
        // Calculate c_init for data scrambling
        // c_init = (rnti << 16) + n_id) mod 2^31
        let n_id = self.cell_config.pci; // Using PCI as scrambling ID
        let c_init = (((rnti as u32) << 16) + n_id as u32) & 0x7FFFFFFF;
        
        // Create scrambling sequence generator
        let mut generator = DmrsSequenceGenerator::new(c_init);
        
        // Generate scrambling sequence and apply
        let mut scrambled = Vec::with_capacity(data.len());
        for &bit in data {
            let c_bit = generator.next_bit();
            scrambled.push(bit ^ c_bit);
        }
        
        scrambled
    }
}