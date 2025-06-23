//! Primary and Secondary Synchronization Signal Generation
//! 
//! Implements PSS and SSS generation according to 3GPP TS 38.211

use crate::LayerError;
use common::types::Pci;
use num_complex::Complex32;
use tracing::debug;

/// PSS sequence length
const PSS_LENGTH: usize = 127;

/// SSS sequence length  
const SSS_LENGTH: usize = 127;

/// m-sequence generator polynomial for PSS
const PSS_POLYNOMIAL: u8 = 0x11; // x^7 + x^4 + 1

/// PSS generator
#[derive(Debug, Clone)]
pub struct PssGenerator {
    /// Physical cell ID (0-1007)
    pci: Pci,
    /// PSS sequence ID (0-2)
    nid2: u8,
    /// Pre-generated PSS sequence
    sequence: Vec<Complex32>,
}

impl PssGenerator {
    /// Create a new PSS generator
    pub fn new(pci: Pci) -> Result<Self, LayerError> {
        // Extract PSS sequence ID from PCI
        let nid2 = (pci.0 % 3) as u8;
        
        // Generate PSS sequence
        let sequence = generate_pss_sequence(nid2);
        
        Ok(Self {
            pci,
            nid2,
            sequence,
        })
    }
    
    /// Generate PSS symbols
    pub fn generate(&self) -> Vec<Complex32> {
        debug!("Generating PSS, sequence length: {}", self.sequence.len());
        self.sequence.clone()
    }
    
    /// Get PSS sequence ID
    pub fn nid2(&self) -> u8 {
        self.nid2
    }
}

/// SSS generator
#[derive(Debug, Clone)]
pub struct SssGenerator {
    /// Physical cell ID (0-1007)
    pci: Pci,
    /// SSS sequence IDs
    nid1: u16,
    nid2: u8,
}

impl SssGenerator {
    /// Create a new SSS generator
    pub fn new(pci: Pci) -> Result<Self, LayerError> {
        // Extract SSS parameters from PCI
        let nid2 = (pci.0 % 3) as u8;
        let nid1 = pci.0 / 3;
        
        Ok(Self {
            pci,
            nid1,
            nid2,
        })
    }
    
    /// Generate SSS symbols for given frame number
    pub fn generate(&self, frame_number: u32) -> Vec<Complex32> {
        // SSS changes based on frame number for frame timing
        let is_subframe_5 = (frame_number % 2) == 1;
        generate_sss_sequence(self.nid1, self.nid2, is_subframe_5)
    }
    
    /// Get cell group ID
    pub fn nid1(&self) -> u16 {
        self.nid1
    }
}

/// Generate PSS sequence
fn generate_pss_sequence(nid2: u8) -> Vec<Complex32> {
    let mut sequence = Vec::with_capacity(PSS_LENGTH);
    
    // Generate m-sequence
    // Initialize with correct state as per 3GPP TS 38.211 and srsRAN
    let mut x = [0u8; 7];
    x[0] = 0;
    x[1] = 1;
    x[2] = 1;
    x[3] = 0;
    x[4] = 1;
    x[5] = 1;
    x[6] = 1;
    
    let mut d_u = Vec::with_capacity(PSS_LENGTH);
    
    for _ in 0..PSS_LENGTH {
        // Output bit - use x[0] as output (consistent with standard)
        d_u.push(x[0]);
        
        // Shift register with feedback
        // Polynomial: x^7 + x^4 + 1
        let feedback = (x[0] + x[3]) % 2;
        
        // Shift left
        for i in 0..6 {
            x[i] = x[i + 1];
        }
        x[6] = feedback;
    }
    
    // Calculate cyclic shift M as per 3GPP TS 38.211
    let m = (43 * nid2 as usize) % PSS_LENGTH;
    
    // Generate BPSK modulated sequence with cyclic shift
    // As per 3GPP: d_pss(n) = 1 - 2*x((n + m) mod 127)
    let amplitude = 1.0;  // Full scale amplitude
    for n in 0..PSS_LENGTH {
        let idx = (n + m) % PSS_LENGTH;
        // BPSK mapping: x=0 -> d=1, x=1 -> d=-1
        let value = amplitude * (1.0 - 2.0 * d_u[idx] as f32);
        sequence.push(Complex32::new(value, 0.0));
    }
    
    sequence
}

/// Generate SSS sequence
fn generate_sss_sequence(nid1: u16, nid2: u8, is_subframe_5: bool) -> Vec<Complex32> {
    let mut sequence = Vec::with_capacity(SSS_LENGTH);
    
    // Generate x0 and x1 m-sequences
    let x0 = generate_m_sequence(0);
    let x1 = generate_m_sequence(1);
    
    // Calculate m0 and m1 based on NID1
    let m0 = (nid1 % 112) as usize;
    let m1 = ((nid1 / 112) + (nid1 % 112) + 1) % 112;
    
    // Generate SSS sequence
    for n in 0..SSS_LENGTH {
        let s0_idx = (n + m0 as usize) % 127;
        let s1_idx = (n + m1 as usize) % 127;
        
        let (s0, s1) = if !is_subframe_5 {
            (x0[s0_idx], x1[s1_idx])
        } else {
            (x1[s1_idx], x0[s0_idx]) // Swapped for subframe 5
        };
        
        // Apply scrambling based on NID2
        let c0 = generate_scrambling_sequence(nid2, 0);
        let c1 = generate_scrambling_sequence(nid2, 1);
        
        let value = (1.0 - 2.0 * s0 as f32) * (1.0 - 2.0 * c0[n] as f32) +
                   (1.0 - 2.0 * s1 as f32) * (1.0 - 2.0 * c1[n] as f32);
        
        sequence.push(Complex32::new(value / 2.0, 0.0));
    }
    
    sequence
}

/// Generate m-sequence for SSS
fn generate_m_sequence(init: u8) -> Vec<u8> {
    let mut sequence = Vec::with_capacity(127);
    // Initialize based on sequence type
    let mut x = if init == 0 {
        [1, 0, 0, 0, 0, 0, 0]
    } else {
        [0, 0, 0, 0, 0, 0, 1]
    };
    
    for _ in 0..127 {
        sequence.push(x[6]);
        
        // Polynomial: x^7 + x^4 + 1
        let feedback = x[6] ^ x[3];
        for i in (1..7).rev() {
            x[i] = x[i - 1];
        }
        x[0] = feedback;
    }
    
    sequence
}

/// Generate scrambling sequence for SSS
fn generate_scrambling_sequence(nid2: u8, c_init_offset: u8) -> Vec<u8> {
    let mut sequence = Vec::with_capacity(127);
    let mut x = [0u8; 7];
    
    // Initialize based on NID2 and offset
    let init_val = (nid2 + c_init_offset) % 127;
    for i in 0..7 {
        x[i] = ((init_val >> i) & 1) as u8;
    }
    
    for _ in 0..127 {
        sequence.push(x[6]);
        
        // Polynomial: x^7 + x^4 + 1
        let feedback = x[6] ^ x[3];
        for i in (1..7).rev() {
            x[i] = x[i - 1];
        }
        x[0] = feedback;
    }
    
    sequence
}

/// Cell search result
#[derive(Debug, Clone)]
pub struct CellSearchResult {
    /// Detected PCI
    pub pci: Pci,
    /// PSS correlation peak
    pub pss_correlation: f32,
    /// SSS correlation peak
    pub sss_correlation: f32,
    /// Timing offset in samples
    pub timing_offset: i32,
    /// Frequency offset in Hz
    pub frequency_offset: f32,
}

/// PSS correlator for cell search
pub struct PssCorrelator {
    /// Reference PSS sequences for all 3 NID2 values
    pss_sequences: [Vec<Complex32>; 3],
}

impl PssCorrelator {
    /// Create a new PSS correlator
    pub fn new() -> Self {
        let pss_sequences = [
            generate_pss_sequence(0),
            generate_pss_sequence(1),
            generate_pss_sequence(2),
        ];
        
        Self { pss_sequences }
    }
    
    /// Correlate input samples with PSS sequences
    pub fn correlate(&self, samples: &[Complex32]) -> Option<(u8, f32, usize)> {
        if samples.len() < PSS_LENGTH {
            return None;
        }
        
        let mut best_nid2 = 0;
        let mut best_correlation = 0.0;
        let mut best_offset = 0;
        
        // Try each PSS sequence
        for (nid2, pss_seq) in self.pss_sequences.iter().enumerate() {
            // Sliding correlation
            for offset in 0..=(samples.len() - PSS_LENGTH) {
                let correlation = self.correlate_at_offset(
                    &samples[offset..offset + PSS_LENGTH],
                    pss_seq,
                );
                
                if correlation > best_correlation {
                    best_correlation = correlation;
                    best_nid2 = nid2 as u8;
                    best_offset = offset;
                }
            }
        }
        
        // Check if correlation is significant
        if best_correlation > 0.5 {
            Some((best_nid2, best_correlation, best_offset))
        } else {
            None
        }
    }
    
    /// Correlate at specific offset
    fn correlate_at_offset(&self, samples: &[Complex32], reference: &[Complex32]) -> f32 {
        let mut correlation = Complex32::new(0.0, 0.0);
        let mut signal_power = 0.0;
        let mut ref_power = 0.0;
        
        for i in 0..PSS_LENGTH {
            correlation += samples[i] * reference[i].conj();
            signal_power += samples[i].norm_sqr();
            ref_power += reference[i].norm_sqr();
        }
        
        // Normalized correlation
        if signal_power > 0.0 && ref_power > 0.0 {
            correlation.norm() / (signal_power * ref_power).sqrt()
        } else {
            0.0
        }
    }
}

/// SSS correlator for cell ID detection
pub struct SssCorrelator {
    /// Maximum NID1 to search
    max_nid1: u16,
}

impl SssCorrelator {
    /// Create a new SSS correlator
    pub fn new() -> Self {
        Self { max_nid1: 335 } // 336 cell groups
    }
    
    /// Detect SSS and determine NID1
    pub fn detect(&self, samples: &[Complex32], nid2: u8, is_subframe_5: bool) -> Option<(u16, f32)> {
        if samples.len() < SSS_LENGTH {
            return None;
        }
        
        let mut best_nid1 = 0;
        let mut best_correlation = 0.0;
        
        // Try each possible NID1
        for nid1 in 0..=self.max_nid1 {
            let sss_seq = generate_sss_sequence(nid1, nid2, is_subframe_5);
            let correlation = self.correlate_sequence(samples, &sss_seq);
            
            if correlation > best_correlation {
                best_correlation = correlation;
                best_nid1 = nid1;
            }
        }
        
        // Check if correlation is significant
        if best_correlation > 0.5 {
            Some((best_nid1, best_correlation))
        } else {
            None
        }
    }
    
    /// Correlate with SSS sequence
    fn correlate_sequence(&self, samples: &[Complex32], reference: &[Complex32]) -> f32 {
        let mut correlation = Complex32::new(0.0, 0.0);
        let mut signal_power = 0.0;
        let mut ref_power = 0.0;
        
        for i in 0..SSS_LENGTH.min(samples.len()) {
            correlation += samples[i] * reference[i].conj();
            signal_power += samples[i].norm_sqr();
            ref_power += reference[i].norm_sqr();
        }
        
        // Normalized correlation
        if signal_power > 0.0 && ref_power > 0.0 {
            correlation.norm() / (signal_power * ref_power).sqrt()
        } else {
            0.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_pss_generator() {
        let pci = Pci::new(0).unwrap();
        let pss_gen = PssGenerator::new(pci).unwrap();
        let pss = pss_gen.generate();
        
        assert_eq!(pss.len(), PSS_LENGTH);
        assert_eq!(pss_gen.nid2(), 0);
    }
    
    #[test]
    fn test_sss_generator() {
        let pci = Pci::new(135).unwrap(); // NID1=45, NID2=0
        let sss_gen = SssGenerator::new(pci).unwrap();
        let sss = sss_gen.generate(0);
        
        assert_eq!(sss.len(), SSS_LENGTH);
        assert_eq!(sss_gen.nid1(), 45);
    }
    
    #[test]
    fn test_pss_correlation() {
        let correlator = PssCorrelator::new();
        
        // Generate test PSS
        let pss_seq = generate_pss_sequence(1);
        
        // Should detect NID2=1
        if let Some((nid2, corr, offset)) = correlator.correlate(&pss_seq) {
            assert_eq!(nid2, 1);
            assert!(corr > 0.99); // Should be perfect correlation
            assert_eq!(offset, 0);
        } else {
            panic!("PSS correlation failed");
        }
    }
}