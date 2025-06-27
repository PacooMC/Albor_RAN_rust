//! Primary and Secondary Synchronization Signal Generation
//! 
//! Implements PSS and SSS generation according to 3GPP TS 38.211

use crate::LayerError;
use common::types::Pci;
use num_complex::Complex32;
use tracing::{debug, info};

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
    /// PSS amplitude (linear scale)
    amplitude: f32,
}

impl PssGenerator {
    /// Create a new PSS generator
    pub fn new(pci: Pci) -> Result<Self, LayerError> {
        // Extract PSS sequence ID from PCI
        let nid2 = (pci.0 % 3) as u8;
        
        // Generate PSS sequence with unit amplitude
        let sequence = generate_pss_sequence(nid2);
        
        // Default amplitude: 1.0 (0 dB) - matches srsRAN default
        // In srsRAN, PSS can be 0 dB or 3 dB higher than SSS
        // We'll use 3 dB to match srsRAN default for proper detection
        let amplitude = 10.0_f32.powf(3.0 / 20.0); // 3 dB = ~1.414
        
        Ok(Self {
            pci,
            nid2,
            sequence,
            amplitude,
        })
    }
    
    /// Create a new PSS generator with specific amplitude in dB
    pub fn new_with_amplitude_db(pci: Pci, amplitude_db: f32) -> Result<Self, LayerError> {
        // Extract PSS sequence ID from PCI
        let nid2 = (pci.0 % 3) as u8;
        
        // Generate PSS sequence
        let sequence = generate_pss_sequence(nid2);
        
        // Convert dB to linear amplitude
        let amplitude = 10.0_f32.powf(amplitude_db / 20.0);
        
        Ok(Self {
            pci,
            nid2,
            sequence,
            amplitude,
        })
    }
    
    /// Generate PSS symbols
    pub fn generate(&self) -> Vec<Complex32> {
        debug!("Generating PSS, sequence length: {}, amplitude: {:.3}", 
               self.sequence.len(), self.amplitude);
        
        // Apply amplitude scaling
        let scaled_sequence: Vec<Complex32> = self.sequence.iter()
            .map(|&s| s * self.amplitude)
            .collect();
        
        // Debug output: print first 10 values to verify against srsRAN
        info!("PSS sequence (NID2={}) first 10 values:", self.nid2);
        for i in 0..10.min(scaled_sequence.len()) {
            info!("  PSS[{}] = {:.6} + {:.6}j", 
                  i, 
                  scaled_sequence[i].re, 
                  scaled_sequence[i].im);
        }
        
        scaled_sequence
    }
    
    /// Get PSS sequence ID
    pub fn nid2(&self) -> u8 {
        self.nid2
    }
    
    /// Set PSS amplitude in dB
    pub fn set_amplitude_db(&mut self, amplitude_db: f32) {
        self.amplitude = 10.0_f32.powf(amplitude_db / 20.0);
        info!("PSS amplitude set to {} dB (linear: {:.3})", amplitude_db, self.amplitude);
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
    /// SSS amplitude (linear scale)
    amplitude: f32,
}

impl SssGenerator {
    /// Create a new SSS generator
    pub fn new(pci: Pci) -> Result<Self, LayerError> {
        // Extract SSS parameters from PCI
        let nid2 = (pci.0 % 3) as u8;
        let nid1 = pci.0 / 3;
        
        // Default amplitude: 1.0 (0 dB) - matches srsRAN
        let amplitude = 1.0;
        
        Ok(Self {
            pci,
            nid1,
            nid2,
            amplitude,
        })
    }
    
    /// Create a new SSS generator with specific amplitude in dB
    pub fn new_with_amplitude_db(pci: Pci, amplitude_db: f32) -> Result<Self, LayerError> {
        // Extract SSS parameters from PCI
        let nid2 = (pci.0 % 3) as u8;
        let nid1 = pci.0 / 3;
        
        // Convert dB to linear amplitude
        let amplitude = 10.0_f32.powf(amplitude_db / 20.0);
        
        Ok(Self {
            pci,
            nid1,
            nid2,
            amplitude,
        })
    }
    
    /// Generate SSS symbols for given frame number
    pub fn generate(&self, frame_number: u32) -> Vec<Complex32> {
        // SSS changes based on frame number for frame timing
        let is_subframe_5 = (frame_number % 2) == 1;
        let sequence = generate_sss_sequence(self.nid1, self.nid2, is_subframe_5);
        
        debug!("Generating SSS, sequence length: {}, amplitude: {:.3}", 
               sequence.len(), self.amplitude);
        
        // Apply amplitude scaling
        let scaled_sequence: Vec<Complex32> = sequence.iter()
            .map(|&s| s * self.amplitude)
            .collect();
        
        // Debug output: print first 10 values to verify against srsRAN
        info!("SSS sequence (NID1={}, NID2={}) first 10 values:", self.nid1, self.nid2);
        for i in 0..10.min(scaled_sequence.len()) {
            info!("  SSS[{}] = {:.6} + {:.6}j", 
                  i, 
                  scaled_sequence[i].re, 
                  scaled_sequence[i].im);
        }
        
        scaled_sequence
    }
    
    /// Set SSS amplitude in dB
    pub fn set_amplitude_db(&mut self, amplitude_db: f32) {
        self.amplitude = 10.0_f32.powf(amplitude_db / 20.0);
        info!("SSS amplitude set to {} dB (linear: {:.3})", amplitude_db, self.amplitude);
    }
    
    /// Get cell group ID
    pub fn nid1(&self) -> u16 {
        self.nid1
    }
}

/// Generate PSS sequence
fn generate_pss_sequence(nid2: u8) -> Vec<Complex32> {
    let mut sequence = Vec::with_capacity(PSS_LENGTH);
    
    // Generate m-sequence exactly as srsRAN does
    // Initialize with state from srsRAN: x[6]=1, x[5]=1, x[4]=1, x[3]=0, x[2]=1, x[1]=1, x[0]=0
    let mut x = vec![0u8; PSS_LENGTH + 7];
    x[6] = 1;
    x[5] = 1;
    x[4] = 1;
    x[3] = 0;
    x[2] = 1;
    x[1] = 1;
    x[0] = 0;
    
    // Log initialization state
    info!("PSS generation for NID2={}: initial state x[6..0] = [{}, {}, {}, {}, {}, {}, {}]",
          nid2, x[6], x[5], x[4], x[3], x[2], x[1], x[0]);
    
    // Generate M sequence x exactly as srsRAN
    for i in 0..PSS_LENGTH {
        x[i + 7] = (x[i + 4] + x[i]) % 2;
    }
    
    // Log first 20 m-sequence values for debugging
    let mut m_seq_debug = String::new();
    for i in 0..20.min(PSS_LENGTH) {
        m_seq_debug.push_str(&format!("{}", x[i]));
    }
    info!("PSS m-sequence (first 20 bits): {}", m_seq_debug);
    
    // Calculate cyclic shift M as per 3GPP TS 38.211
    let m_shift = (43 * nid2 as usize) % PSS_LENGTH;
    info!("PSS cyclic shift M = 43 * {} mod 127 = {}", nid2, m_shift);
    
    // Generate BPSK modulated sequence with cyclic shift applied during output
    // This matches srsRAN's approach exactly
    // Match srsRAN implementation - no scaling at generation
    let amplitude = 1.0;  // Match srsRAN implementation - no scaling at generation
    for n in 0..PSS_LENGTH {
        // Apply cyclic shift when reading from m-sequence (srsRAN approach)
        let m = (n + m_shift) % PSS_LENGTH;
        // BPSK mapping: x=0 -> d=1, x=1 -> d=-1
        let value = amplitude * (1.0 - 2.0 * x[m] as f32);
        sequence.push(Complex32::new(value, 0.0));
    }
    
    // Log first and last 10 PSS values for verification
    info!("PSS sequence (first 10 values):");
    for i in 0..10.min(sequence.len()) {
        info!("  PSS[{}] = {:.3}", i, sequence[i].re);
    }
    info!("PSS sequence (last 10 values):");
    for i in (sequence.len().saturating_sub(10))..sequence.len() {
        info!("  PSS[{}] = {:.3}", i, sequence[i].re);
    }
    
    sequence
}

/// Generate SSS sequence
fn generate_sss_sequence(nid1: u16, nid2: u8, _is_subframe_5: bool) -> Vec<Complex32> {
    let mut sequence = Vec::with_capacity(SSS_LENGTH);
    
    // Generate base sequences exactly as srsRAN does
    let (sequence0, sequence1) = generate_sss_base_sequences();
    
    // Calculate m0 and m1 exactly as srsRAN
    // m0 = 15 * (NID_1 / 112) + 5 * NID_2
    // m1 = NID_1 % 112
    let m0 = (15 * (nid1 / 112) + 5 * nid2 as u16) as usize;
    let m1 = (nid1 % 112) as usize;
    
    // Generate SSS by applying cyclic shifts and element-wise multiplication
    // This matches srsRAN's approach exactly
    for n in 0..SSS_LENGTH {
        // Apply d0 sequence with cyclic shift m0
        let idx0 = (n + m0) % SSS_LENGTH;
        let d0_value = sequence0[idx0];
        
        // Apply d1 sequence with cyclic shift m1
        let idx1 = (n + m1) % SSS_LENGTH;
        let d1_value = sequence1[idx1];
        
        // Element-wise multiplication (as srsRAN does)
        let value = d0_value * d1_value;
        sequence.push(Complex32::new(value, 0.0));
    }
    
    sequence
}

/// Generate SSS base sequences exactly as srsRAN
fn generate_sss_base_sequences() -> (Vec<f32>, Vec<f32>) {
    let mut sequence0 = Vec::with_capacity(SSS_LENGTH);
    let mut sequence1 = Vec::with_capacity(SSS_LENGTH);
    
    // Initialize M sequence x0
    let mut x0 = vec![0u8; SSS_LENGTH + 7];
    x0[6] = 0;
    x0[5] = 0;
    x0[4] = 0;
    x0[3] = 0;
    x0[2] = 0;
    x0[1] = 0;
    x0[0] = 1;
    
    info!("SSS x0 initial state: [{}, {}, {}, {}, {}, {}, {}]", 
          x0[6], x0[5], x0[4], x0[3], x0[2], x0[1], x0[0]);
    
    // Generate M sequence x0 with polynomial x^7 + x^4 + 1
    for i in 0..SSS_LENGTH {
        x0[i + 7] = (x0[i + 4] + x0[i]) % 2;
    }
    
    // Log first 20 x0 sequence values
    let mut x0_debug = String::new();
    for i in 0..20.min(SSS_LENGTH) {
        x0_debug.push_str(&format!("{}", x0[i]));
    }
    info!("SSS x0 sequence (first 20 bits): {}", x0_debug);
    
    // Modulate M sequence to create d0
    // Match srsRAN implementation - no scaling at generation
    let amplitude = 1.0;  // Match srsRAN implementation - no scaling at generation
    for i in 0..SSS_LENGTH {
        sequence0.push(amplitude * (1.0 - 2.0 * x0[i] as f32));
    }
    
    // Initialize M sequence x1
    let mut x1 = vec![0u8; SSS_LENGTH + 7];
    x1[6] = 0;
    x1[5] = 0;
    x1[4] = 0;
    x1[3] = 0;
    x1[2] = 0;
    x1[1] = 0;
    x1[0] = 1;
    
    info!("SSS x1 initial state: [{}, {}, {}, {}, {}, {}, {}]", 
          x1[6], x1[5], x1[4], x1[3], x1[2], x1[1], x1[0]);
    
    // Generate M sequence x1 with polynomial x^7 + x + 1
    for i in 0..SSS_LENGTH {
        x1[i + 7] = (x1[i + 1] + x1[i]) % 2;
    }
    
    // Log first 20 x1 sequence values
    let mut x1_debug = String::new();
    for i in 0..20.min(SSS_LENGTH) {
        x1_debug.push_str(&format!("{}", x1[i]));
    }
    info!("SSS x1 sequence (first 20 bits): {}", x1_debug);
    
    // Modulate M sequence to create d1
    // Match srsRAN implementation - no scaling at generation
    let amplitude = 1.0;  // Match srsRAN implementation - no scaling at generation
    for i in 0..SSS_LENGTH {
        sequence1.push(amplitude * (1.0 - 2.0 * x1[i] as f32));
    }
    
    (sequence0, sequence1)
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