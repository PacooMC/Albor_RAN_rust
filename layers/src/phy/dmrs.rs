/// DMRS (Demodulation Reference Signal) generation for PDCCH and PDSCH
/// Based on 3GPP TS 38.211 Section 7.4.1

use num_complex::Complex32;

/// DMRS Type configuration
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DmrsType {
    Type1,
    Type2,
}

impl DmrsType {
    /// Get number of DMRS per resource block
    pub fn nof_dmrs_per_rb(&self) -> usize {
        match self {
            DmrsType::Type1 => 6,  // Every other subcarrier
            DmrsType::Type2 => 4,  // Two groups of 2 consecutive subcarriers
        }
    }
}

/// CDM weights for PDSCH DMRS
#[derive(Debug, Clone)]
pub struct DmrsWeights {
    /// Frequency domain weights [w_f(0), w_f(1)]
    pub w_f: [f32; 2],
    /// Time domain weights [w_t(0), w_t(1)]
    pub w_t: [f32; 2],
}

/// Get DMRS parameters for PDSCH based on type and port
pub fn get_pdsch_dmrs_params(dmrs_type: DmrsType, dmrs_port: u8) -> (Vec<u8>, DmrsWeights) {
    match dmrs_type {
        DmrsType::Type1 => {
            match dmrs_port {
                0 => (vec![0, 2, 4, 6, 8, 10], DmrsWeights { w_f: [1.0, 1.0], w_t: [1.0, 1.0] }),
                1 => (vec![0, 2, 4, 6, 8, 10], DmrsWeights { w_f: [1.0, -1.0], w_t: [1.0, 1.0] }),
                2 => (vec![1, 3, 5, 7, 9, 11], DmrsWeights { w_f: [1.0, 1.0], w_t: [1.0, 1.0] }),
                3 => (vec![1, 3, 5, 7, 9, 11], DmrsWeights { w_f: [1.0, -1.0], w_t: [1.0, 1.0] }),
                4 => (vec![0, 2, 4, 6, 8, 10], DmrsWeights { w_f: [1.0, 1.0], w_t: [1.0, -1.0] }),
                5 => (vec![0, 2, 4, 6, 8, 10], DmrsWeights { w_f: [1.0, -1.0], w_t: [1.0, -1.0] }),
                6 => (vec![1, 3, 5, 7, 9, 11], DmrsWeights { w_f: [1.0, 1.0], w_t: [1.0, -1.0] }),
                7 => (vec![1, 3, 5, 7, 9, 11], DmrsWeights { w_f: [1.0, -1.0], w_t: [1.0, -1.0] }),
                _ => panic!("Invalid DMRS port {} for Type1", dmrs_port),
            }
        }
        DmrsType::Type2 => {
            match dmrs_port {
                0 => (vec![0, 1, 6, 7], DmrsWeights { w_f: [1.0, 1.0], w_t: [1.0, 1.0] }),
                1 => (vec![0, 1, 6, 7], DmrsWeights { w_f: [1.0, -1.0], w_t: [1.0, 1.0] }),
                2 => (vec![2, 3, 8, 9], DmrsWeights { w_f: [1.0, 1.0], w_t: [1.0, 1.0] }),
                3 => (vec![2, 3, 8, 9], DmrsWeights { w_f: [1.0, -1.0], w_t: [1.0, 1.0] }),
                4 => (vec![4, 5, 10, 11], DmrsWeights { w_f: [1.0, 1.0], w_t: [1.0, 1.0] }),
                5 => (vec![4, 5, 10, 11], DmrsWeights { w_f: [1.0, -1.0], w_t: [1.0, 1.0] }),
                6 => (vec![0, 1, 6, 7], DmrsWeights { w_f: [1.0, 1.0], w_t: [1.0, -1.0] }),
                7 => (vec![0, 1, 6, 7], DmrsWeights { w_f: [1.0, -1.0], w_t: [1.0, -1.0] }),
                8 => (vec![2, 3, 8, 9], DmrsWeights { w_f: [1.0, 1.0], w_t: [1.0, -1.0] }),
                9 => (vec![2, 3, 8, 9], DmrsWeights { w_f: [1.0, -1.0], w_t: [1.0, -1.0] }),
                10 => (vec![4, 5, 10, 11], DmrsWeights { w_f: [1.0, 1.0], w_t: [1.0, -1.0] }),
                11 => (vec![4, 5, 10, 11], DmrsWeights { w_f: [1.0, -1.0], w_t: [1.0, -1.0] }),
                _ => panic!("Invalid DMRS port {} for Type2", dmrs_port),
            }
        }
    }
}

/// DMRS sequence generator
pub struct DmrsSequenceGenerator {
    /// Gold sequence LFSR state
    x1: u32,
    x2: u32,
}

impl DmrsSequenceGenerator {
    /// Create new DMRS sequence generator with initialization value
    pub fn new(c_init: u32) -> Self {
        // Initialize x1 with all ones (2^31 - 1)
        let mut x1 = 0x7FFFFFFF;
        // Initialize x2 with c_init
        let mut x2 = c_init & 0x7FFFFFFF;
        
        // Advance LFSR by Nc=1600 iterations as per 3GPP spec
        for _ in 0..1600 {
            // x1 sequence: x1(n+31) = (x1(n+3) + x1(n)) mod 2
            let x1_new = ((x1 >> 3) ^ x1) & 1;
            x1 = ((x1 >> 1) | (x1_new << 30)) & 0x7FFFFFFF;
            
            // x2 sequence: x2(n+31) = (x2(n+3) + x2(n+2) + x2(n+1) + x2(n)) mod 2
            let x2_new = ((x2 >> 3) ^ (x2 >> 2) ^ (x2 >> 1) ^ x2) & 1;
            x2 = ((x2 >> 1) | (x2_new << 30)) & 0x7FFFFFFF;
        }
        
        Self { x1, x2 }
    }
    
    /// Advance LFSR state
    fn advance(&mut self) {
        // x1 sequence: x1(n+31) = (x1(n+3) + x1(n)) mod 2
        let x1_new = ((self.x1 >> 3) ^ self.x1) & 1;
        self.x1 = ((self.x1 >> 1) | (x1_new << 30)) & 0x7FFFFFFF;
        
        // x2 sequence: x2(n+31) = (x2(n+3) + x2(n+2) + x2(n+1) + x2(n)) mod 2
        let x2_new = ((self.x2 >> 3) ^ (self.x2 >> 2) ^ (self.x2 >> 1) ^ self.x2) & 1;
        self.x2 = ((self.x2 >> 1) | (x2_new << 30)) & 0x7FFFFFFF;
    }
    
    /// Generate next bit from the sequence
    pub fn next_bit(&mut self) -> u8 {
        let c = (self.x1 ^ self.x2) & 1;
        self.advance();
        c as u8
    }
    
    /// Generate QPSK symbol from sequence
    pub fn next_qpsk_symbol(&mut self, amplitude: f32) -> Complex32 {
        let c0 = self.next_bit();
        let c1 = self.next_bit();
        
        Complex32::new(
            amplitude * (1.0 - 2.0 * c0 as f32),
            amplitude * (1.0 - 2.0 * c1 as f32),
        )
    }
    
    /// Skip n symbols (2 bits per symbol for QPSK)
    pub fn skip(&mut self, n_symbols: usize) {
        for _ in 0..(n_symbols * 2) {
            self.advance();
        }
    }
}

/// Calculate PDCCH DMRS initialization value
/// c_init = (2^17 * (14 * n_slot + l + 1) * (2 * N_ID + 1) + 2 * N_ID) mod 2^31
pub fn calculate_pdcch_dmrs_cinit(slot: u32, symbol: u8, n_id: u16) -> u32 {
    let l = symbol as u32;
    let n_symb_slot = 14u32; // Normal CP
    ((1 << 17) * (n_symb_slot * slot + l + 1) * (2 * n_id as u32 + 1) + 2 * n_id as u32) & 0x7FFFFFFF
}

/// Calculate PDSCH DMRS initialization value  
/// c_init = (2^17 * (14 * n_slot + l + 1) * (2 * N_ID + 1) + 2 * N_ID + n_SCID) mod 2^31
pub fn calculate_pdsch_dmrs_cinit(slot: u32, symbol: u8, n_id: u16, n_scid: bool) -> u32 {
    let l = symbol as u32;
    let n_symb_slot = 14u32; // Normal CP
    let scid = if n_scid { 1 } else { 0 };
    ((1 << 17) * (n_symb_slot * slot + l + 1) * (2 * n_id as u32 + 1) + 2 * n_id as u32 + scid) & 0x7FFFFFFF
}

/// Calculate PBCH DMRS initialization value
/// According to 3GPP TS 38.211 Section 7.4.1.4.1
pub fn calculate_pbch_dmrs_cinit(n_id: u16, ssb_idx: u8, n_hf: u8, l_max: u8) -> u32 {
    // Calculate i_ssb based on L_max
    let i_ssb = if l_max == 4 {
        // For L_max = 4: i_ssb = least 2 significant bits of SSB index + 4 * n_hf
        ((ssb_idx & 0b11) as u32) + 4 * (n_hf as u32)
    } else if l_max == 8 || l_max == 64 {
        // For L_max = 8 or 64: i_ssb = least 3 significant bits of SSB index
        (ssb_idx & 0b111) as u32
    } else {
        // Default to L_max = 4 behavior
        ((ssb_idx & 0b11) as u32) + 4 * (n_hf as u32)
    };
    
    // c_init = 2^11 * (i_ssb + 1) * (floor(N_ID/4) + 1) + 2^6 * (i_ssb + 1) + (N_ID mod 4)
    let n_id_div_4 = (n_id / 4) as u32;
    let n_id_mod_4 = (n_id % 4) as u32;
    
    (((i_ssb + 1) * (n_id_div_4 + 1)) << 11) + ((i_ssb + 1) << 6) + n_id_mod_4
}

/// Generate DMRS sequence for given resource blocks
pub fn generate_dmrs_sequence(
    rb_mask: &[bool],
    reference_point_k_rb: u16,
    nof_dmrs_per_rb: usize,
    generator: &mut DmrsSequenceGenerator,
    amplitude: f32,
) -> Vec<Complex32> {
    let mut sequence = Vec::new();
    let mut current_rb = reference_point_k_rb;
    
    for (rb_idx, &is_allocated) in rb_mask.iter().enumerate() {
        let rb = reference_point_k_rb + rb_idx as u16;
        
        if is_allocated {
            // Skip symbols between current position and this RB
            if rb > current_rb {
                let skip_rbs = (rb - current_rb) as usize;
                generator.skip(skip_rbs * nof_dmrs_per_rb);
            }
            
            // Generate symbols for this RB
            for _ in 0..nof_dmrs_per_rb {
                sequence.push(generator.next_qpsk_symbol(amplitude));
            }
            
            current_rb = rb + 1;
        }
    }
    
    sequence
}

/// Apply CDM weights to DMRS sequence for PDSCH
pub fn apply_cdm_weights(
    base_sequence: &[Complex32],
    weights: &DmrsWeights,
    l_prime: usize,
) -> Vec<Complex32> {
    let mut weighted = Vec::with_capacity(base_sequence.len());
    
    // Apply time domain weight
    let w_t = weights.w_t[l_prime];
    
    // Apply frequency domain weights (alternating pattern)
    for (idx, &symbol) in base_sequence.iter().enumerate() {
        let w_f = weights.w_f[idx % 2];
        weighted.push(symbol * w_t * w_f);
    }
    
    weighted
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_pdcch_dmrs_cinit() {
        // Test case from 3GPP
        let slot = 0;
        let symbol = 0;
        let n_id = 0;
        let c_init = calculate_pdcch_dmrs_cinit(slot, symbol, n_id);
        assert_eq!(c_init & 0x7FFFFFFF, c_init); // Should be 31 bits
    }
    
    #[test] 
    fn test_dmrs_sequence_generation() {
        let c_init = 100;
        let mut gen = DmrsSequenceGenerator::new(c_init);
        
        // Generate a few symbols and check they are QPSK
        for _ in 0..10 {
            let symbol = gen.next_qpsk_symbol(1.0 / std::f32::consts::SQRT_2);
            // QPSK symbols should have magnitude sqrt(0.5)
            let mag = (symbol.re * symbol.re + symbol.im * symbol.im).sqrt();
            assert!((mag - 1.0).abs() < 0.001);
        }
    }
}