//! PRACH (Physical Random Access Channel) Implementation
//! 
//! Implements PRACH detection according to 3GPP TS 38.211
//! Detects PRACH preambles sent by UEs during random access procedure

use crate::LayerError;
use common::types::CellId;
use rustfft::{FftPlanner, num_complex::Complex32, Fft};
use std::sync::Arc;
use tracing::{debug, info, trace};

/// PRACH constants according to 3GPP
pub mod constants {
    /// Long sequence length (for formats 0-3)
    pub const LONG_SEQUENCE_LENGTH: usize = 839;
    /// Short sequence length (for formats A1-C2)
    pub const SHORT_SEQUENCE_LENGTH: usize = 139;
    /// Maximum number of preambles
    pub const MAX_NUM_PREAMBLES: usize = 64;
    /// Maximum number of root sequences
    pub const MAX_NUM_ROOT_SEQUENCES: usize = 838;
}

/// PRACH format type
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PrachFormat {
    /// Format 0: 839 sequence length, 1ms duration
    Format0,
    /// Format 1: 839 sequence length, 2ms duration
    Format1,
    /// Format 2: 839 sequence length, 4ms duration
    Format2,
    /// Format 3: 839 sequence length, 4ms duration
    Format3,
    /// Format A1: 139 sequence length (short)
    FormatA1,
    /// Format A2: 139 sequence length (short)
    FormatA2,
    /// Format A3: 139 sequence length (short)
    FormatA3,
    /// Format B1: 139 sequence length (short)
    FormatB1,
    /// Format B4: 139 sequence length (short)
    FormatB4,
    /// Format C0: 139 sequence length (short)
    FormatC0,
    /// Format C2: 139 sequence length (short)
    FormatC2,
}

impl PrachFormat {
    /// Check if this is a long preamble format
    pub fn is_long(&self) -> bool {
        matches!(self, Self::Format0 | Self::Format1 | Self::Format2 | Self::Format3)
    }
    
    /// Get sequence length for this format
    pub fn sequence_length(&self) -> usize {
        if self.is_long() {
            constants::LONG_SEQUENCE_LENGTH
        } else {
            constants::SHORT_SEQUENCE_LENGTH
        }
    }
    
    /// Get number of PRACH symbols
    pub fn num_symbols(&self) -> usize {
        match self {
            Self::Format0 => 1,
            Self::Format1 => 2,
            Self::Format2 => 4,
            Self::Format3 => 4,
            Self::FormatA1 => 2,
            Self::FormatA2 => 4,
            Self::FormatA3 => 6,
            Self::FormatB1 => 2,
            Self::FormatB4 => 12,
            Self::FormatC0 => 1,
            Self::FormatC2 => 4,
        }
    }
}

/// Restricted set configuration
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RestrictedSetConfig {
    UnrestrictedSet,
    RestrictedSetTypeA,
    RestrictedSetTypeB,
}

/// PRACH configuration index entry from 3GPP tables
#[derive(Debug, Clone)]
pub struct PrachConfigurationIndex {
    /// PRACH format
    pub format: PrachFormat,
    /// System frame period (x)
    pub x: u32,
    /// System frame offsets (y)
    pub y: Vec<u8>,
    /// Subframe numbers within a radio frame
    pub subframe_numbers: Vec<u8>,
    /// Starting symbol
    pub starting_symbol: u8,
    /// Number of PRACH slots within a subframe
    pub num_prach_slots_within_subframe: u8,
    /// Number of time-domain PRACH occasions within a PRACH slot
    pub num_occasions_within_slot: u8,
    /// PRACH duration in symbols
    pub duration: u8,
}

/// PRACH subcarrier spacing
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PrachSubcarrierSpacing {
    /// 1.25 kHz (long sequences)
    Khz1_25,
    /// 5 kHz (short sequences) 
    Khz5,
}

/// Get PRACH configuration for FDD from Table 6.3.3.2-2
fn get_prach_config_fdd(index: u8) -> Option<PrachConfigurationIndex> {
    match index {
        0 => Some(PrachConfigurationIndex {
            format: PrachFormat::Format0,
            x: 16,
            y: vec![1],
            subframe_numbers: vec![9],
            starting_symbol: 0,
            num_prach_slots_within_subframe: 1,
            num_occasions_within_slot: 1,
            duration: 1,
        }),
        1 => Some(PrachConfigurationIndex {
            format: PrachFormat::Format0,
            x: 8,
            y: vec![1],
            subframe_numbers: vec![9],
            starting_symbol: 0,
            num_prach_slots_within_subframe: 1,
            num_occasions_within_slot: 1,
            duration: 1,
        }),
        2 => Some(PrachConfigurationIndex {
            format: PrachFormat::Format0,
            x: 4,
            y: vec![1],
            subframe_numbers: vec![9],
            starting_symbol: 0,
            num_prach_slots_within_subframe: 1,
            num_occasions_within_slot: 1,
            duration: 1,
        }),
        3 => Some(PrachConfigurationIndex {
            format: PrachFormat::Format0,
            x: 2,
            y: vec![0],
            subframe_numbers: vec![9],
            starting_symbol: 0,
            num_prach_slots_within_subframe: 1,
            num_occasions_within_slot: 1,
            duration: 1,
        }),
        4 => Some(PrachConfigurationIndex {
            format: PrachFormat::Format0,
            x: 2,
            y: vec![1],
            subframe_numbers: vec![9],
            starting_symbol: 0,
            num_prach_slots_within_subframe: 1,
            num_occasions_within_slot: 1,
            duration: 1,
        }),
        5 => Some(PrachConfigurationIndex {
            format: PrachFormat::Format0,
            x: 2,
            y: vec![0, 1],
            subframe_numbers: vec![9],
            starting_symbol: 0,
            num_prach_slots_within_subframe: 1,
            num_occasions_within_slot: 1,
            duration: 1,
        }),
        6 => Some(PrachConfigurationIndex {
            format: PrachFormat::Format0,
            x: 1,
            y: vec![0],
            subframe_numbers: vec![9],
            starting_symbol: 0,
            num_prach_slots_within_subframe: 1,
            num_occasions_within_slot: 1,
            duration: 1,
        }),
        7 => Some(PrachConfigurationIndex {
            format: PrachFormat::Format0,
            x: 1,
            y: vec![0],
            subframe_numbers: vec![8, 9],
            starting_symbol: 0,
            num_prach_slots_within_subframe: 1,
            num_occasions_within_slot: 1,
            duration: 1,
        }),
        // Add more entries as needed
        _ => None,
    }
}

/// RACH configuration common
#[derive(Debug, Clone)]
pub struct RachConfigCommon {
    /// PRACH configuration index (0-255)
    pub prach_config_index: u8,
    /// Message 2 window length in slots
    pub ra_response_window: u32,
    /// Number of FDMed PRACH occasions
    pub msg1_fdm: u32,
    /// PRACH frequency start offset
    pub msg1_frequency_start: u32,
    /// Zero correlation zone config
    pub zero_correlation_zone_config: u16,
    /// Target received power in dBm
    pub preamble_rx_target_power: i16,
    /// Max preamble transmission attempts
    pub preamble_trans_max: u8,
    /// Power ramping step in dB
    pub power_ramping_step_db: u8,
    /// Total number of RA preambles
    pub total_num_ra_preambles: u8,
    /// PRACH root sequence index
    pub prach_root_seq_index: u16,
    /// PRACH subcarrier spacing
    pub msg1_scs: PrachSubcarrierSpacing,
    /// Restricted set configuration
    pub restricted_set: RestrictedSetConfig,
}

impl Default for RachConfigCommon {
    fn default() -> Self {
        Self {
            prach_config_index: 0,  // Configuration 0 for FDD
            ra_response_window: 10,  // 10 slots
            msg1_fdm: 1,  // Single PRACH occasion in frequency
            msg1_frequency_start: 0,  // Start from PRB 0
            zero_correlation_zone_config: 12,  // Common value
            preamble_rx_target_power: -104,  // -104 dBm
            preamble_trans_max: 7,  // 7 attempts
            power_ramping_step_db: 4,  // 4 dB steps
            total_num_ra_preambles: 64,  // All 64 preambles
            prach_root_seq_index: 0,  // Root sequence 0
            msg1_scs: PrachSubcarrierSpacing::Khz1_25,  // 1.25 kHz for long preambles
            restricted_set: RestrictedSetConfig::UnrestrictedSet,
        }
    }
}

/// Detection result for a single preamble
#[derive(Debug, Clone)]
pub struct PreambleDetection {
    /// Detected preamble index (0-63)
    pub preamble_index: u8,
    /// Timing advance in samples
    pub timing_advance_samples: u32,
    /// Timing advance in microseconds
    pub timing_advance_us: f32,
    /// Detection metric (normalized)
    pub detection_metric: f32,
    /// Received power in dBm
    pub power_dbm: f32,
}

/// PRACH detection result
#[derive(Debug, Clone)]
pub struct PrachDetectionResult {
    /// Frame number
    pub frame: u32,
    /// Slot number
    pub slot: u8,
    /// Average RSSI in dBm
    pub rssi_dbm: f32,
    /// Detected preambles
    pub preambles: Vec<PreambleDetection>,
    /// Time resolution in microseconds
    pub time_resolution_us: f32,
    /// Maximum timing advance in microseconds
    pub max_timing_advance_us: f32,
}

/// PRACH detector
pub struct PrachDetector {
    /// Cell ID
    cell_id: CellId,
    /// RACH configuration
    rach_config: RachConfigCommon,
    /// FFT planner for correlation
    fft_planner: FftPlanner<f32>,
    /// IDFT processor for long preambles
    idft_long: Arc<dyn Fft<f32>>,
    /// IDFT processor for short preambles
    idft_short: Arc<dyn Fft<f32>>,
    /// Pre-generated root sequences
    root_sequences: Vec<Vec<Complex32>>,
}

impl PrachDetector {
    /// Create a new PRACH detector
    pub fn new(cell_id: CellId, rach_config: RachConfigCommon) -> Result<Self, LayerError> {
        let mut fft_planner = FftPlanner::new();
        
        // Create IDFT processors
        let idft_long = fft_planner.plan_fft_inverse(2048);  // Next power of 2 after 839
        let idft_short = fft_planner.plan_fft_inverse(256);  // Next power of 2 after 139
        
        // Pre-generate root sequences
        let root_sequences = Vec::new();  // Will be generated on demand
        
        Ok(Self {
            cell_id,
            rach_config,
            fft_planner,
            idft_long,
            idft_short,
            root_sequences,
        })
    }
    
    /// Generate Zadoff-Chu sequence
    /// x_u(n) = exp(-j * pi * u * n * (n + 1) / N_zc)
    fn generate_zc_sequence(root: u16, length: usize) -> Vec<Complex32> {
        let mut sequence = vec![Complex32::new(0.0, 0.0); length];
        let n_zc = length as f32;
        let u = root as f32;
        
        for n in 0..length {
            let n_f = n as f32;
            let phase = -std::f32::consts::PI * u * n_f * (n_f + 1.0) / n_zc;
            sequence[n] = Complex32::from_polar(1.0, phase);
        }
        
        sequence
    }
    
    /// Get cyclic shift value N_cs from zero correlation zone config
    fn get_cyclic_shift(&self) -> u32 {
        // Simplified - use table lookup in real implementation
        // For FDD with unrestricted set, common values:
        match self.rach_config.zero_correlation_zone_config {
            0 => 0,    // N_cs = 0 (no cyclic shift)
            1 => 13,   // N_cs = 13
            2 => 15,   // N_cs = 15
            3 => 18,   // N_cs = 18
            4 => 22,   // N_cs = 22
            5 => 26,   // N_cs = 26
            6 => 32,   // N_cs = 32
            7 => 38,   // N_cs = 38
            8 => 46,   // N_cs = 46
            9 => 59,   // N_cs = 59
            10 => 76,  // N_cs = 76
            11 => 93,  // N_cs = 93
            12 => 119, // N_cs = 119
            13 => 167, // N_cs = 167
            14 => 279, // N_cs = 279
            15 => 419, // N_cs = 419
            _ => 119,  // Default
        }
    }
    
    /// Check if PRACH is scheduled in this slot
    pub fn is_prach_occasion(&self, frame: u32, slot: u8) -> bool {
        // Get PRACH configuration
        let config = match get_prach_config_fdd(self.rach_config.prach_config_index) {
            Some(c) => c,
            None => return false,
        };
        
        // Check system frame
        let frame_in_period = frame % config.x;
        if !config.y.contains(&(frame_in_period as u8)) {
            return false;
        }
        
        // For FDD, PRACH is in specific subframes
        // Convert slot to subframe (assuming 15 kHz SCS)
        let subframe = slot;  // 1 slot per subframe for 15 kHz
        config.subframe_numbers.contains(&subframe)
    }
    
    /// Detect PRACH preambles in received samples
    pub fn detect(
        &mut self,
        samples: &[Complex32],
        frame: u32,
        slot: u8,
    ) -> Result<PrachDetectionResult, LayerError> {
        // Check if this is a PRACH occasion
        if !self.is_prach_occasion(frame, slot) {
            return Ok(PrachDetectionResult {
                frame,
                slot,
                rssi_dbm: -140.0,
                preambles: Vec::new(),
                time_resolution_us: 0.0,
                max_timing_advance_us: 0.0,
            });
        }
        
        info!("PRACH occasion detected at frame={}, slot={}", frame, slot);
        
        // Get PRACH configuration
        let config = get_prach_config_fdd(self.rach_config.prach_config_index)
            .ok_or_else(|| LayerError::InvalidConfiguration(
                format!("Invalid PRACH config index: {}", self.rach_config.prach_config_index)
            ))?;
        
        // Get sequence length
        let seq_length = config.format.sequence_length();
        let is_long = config.format.is_long();
        
        // Calculate RSSI
        let rssi = samples.iter()
            .map(|s| s.norm_sqr())
            .sum::<f32>() / samples.len() as f32;
        let rssi_dbm = 10.0 * rssi.log10();
        
        // Early exit if signal is too weak
        if rssi_dbm < -120.0 {
            trace!("PRACH signal too weak: {} dBm", rssi_dbm);
            return Ok(PrachDetectionResult {
                frame,
                slot,
                rssi_dbm,
                preambles: Vec::new(),
                time_resolution_us: 0.0,
                max_timing_advance_us: 0.0,
            });
        }
        
        // Generate root sequences if not already done
        if self.root_sequences.is_empty() {
            self.generate_root_sequences(seq_length)?;
        }
        
        // Detect preambles
        let mut detected_preambles = Vec::new();
        
        // Get cyclic shift
        let n_cs = self.get_cyclic_shift();
        let num_shifts = if n_cs > 0 {
            (seq_length as u32 / n_cs).min(64) as usize
        } else {
            1
        };
        
        // Process each root sequence
        let num_sequences = (64 + num_shifts - 1) / num_shifts;
        
        for seq_idx in 0..num_sequences.min(self.root_sequences.len()) {
            // Correlate with root sequence
            let correlation = self.correlate_sequence(
                samples,
                &self.root_sequences[seq_idx],
                is_long
            )?;
            
            // Find peaks for each cyclic shift
            for shift_idx in 0..num_shifts {
                let preamble_idx = seq_idx * num_shifts + shift_idx;
                if preamble_idx >= 64 {
                    break;
                }
                
                // Calculate window for this shift
                let window_start = if n_cs > 0 {
                    (shift_idx as u32 * n_cs) as usize
                } else {
                    0
                };
                
                // Find peak in correlation window
                if let Some((peak_idx, peak_value)) = self.find_correlation_peak(
                    &correlation,
                    window_start,
                    n_cs as usize
                ) {
                    // Check if peak exceeds threshold
                    let threshold = self.calculate_detection_threshold(rssi);
                    if peak_value > threshold {
                        // Calculate timing advance
                        let ta_samples = peak_idx as u32;
                        let sample_rate = 30.72e6;  // 30.72 MHz for 20 MHz BW
                        let ta_us = (ta_samples as f32 * 1e6) / sample_rate as f32;
                        
                        // Calculate power
                        let power_dbm = 10.0 * peak_value.log10();
                        
                        detected_preambles.push(PreambleDetection {
                            preamble_index: preamble_idx as u8,
                            timing_advance_samples: ta_samples,
                            timing_advance_us: ta_us,
                            detection_metric: peak_value / threshold,
                            power_dbm,
                        });
                        
                        info!(
                            "Detected preamble {}: TA={:.1}us, metric={:.2}, power={:.1}dBm",
                            preamble_idx, ta_us, peak_value / threshold, power_dbm
                        );
                    }
                }
            }
        }
        
        // Sort by detection metric
        detected_preambles.sort_by(|a, b| b.detection_metric.partial_cmp(&a.detection_metric).unwrap());
        
        Ok(PrachDetectionResult {
            frame,
            slot,
            rssi_dbm,
            preambles: detected_preambles,
            time_resolution_us: 1e6 / 30.72e6,  // ~32.55 ns
            max_timing_advance_us: (n_cs as f32 * 1e6) / (1.25e3 * seq_length as f32),
        })
    }
    
    /// Generate all root sequences
    fn generate_root_sequences(&mut self, seq_length: usize) -> Result<(), LayerError> {
        // Generate root sequences starting from configured index
        let mut root = self.rach_config.prach_root_seq_index;
        let max_roots = if seq_length == constants::LONG_SEQUENCE_LENGTH {
            838
        } else {
            138
        };
        
        // Generate up to 64 sequences
        for _ in 0..64 {
            let sequence = Self::generate_zc_sequence(root, seq_length);
            self.root_sequences.push(sequence);
            
            // Next root sequence (with wraparound)
            root = (root + 1) % max_roots;
        }
        
        debug!("Generated {} root sequences starting from index {}", 
               self.root_sequences.len(), self.rach_config.prach_root_seq_index);
        
        Ok(())
    }
    
    /// Correlate received signal with root sequence
    fn correlate_sequence(
        &self,
        samples: &[Complex32],
        root_seq: &[Complex32],
        is_long: bool,
    ) -> Result<Vec<f32>, LayerError> {
        let seq_length = root_seq.len();
        let idft = if is_long { &self.idft_long } else { &self.idft_short };
        let idft_size = if is_long { 2048 } else { 256 };
        
        // Prepare IDFT input buffer
        let mut idft_buffer = vec![Complex32::new(0.0, 0.0); idft_size];
        
        // Take first seq_length samples and multiply by conjugate of root sequence
        let signal_len = samples.len().min(seq_length);
        for i in 0..signal_len {
            let prod = samples[i] * root_seq[i].conj();
            
            // Map to IDFT input (frequency domain arrangement)
            if i < seq_length / 2 + 1 {
                idft_buffer[i] = prod;
            } else {
                idft_buffer[idft_size - (seq_length - i)] = prod;
            }
        }
        
        // Perform IDFT to get time-domain correlation (in-place)
        idft.process(&mut idft_buffer);
        
        // Calculate power and normalize
        let norm_factor = 1.0 / (idft_size as f32 * seq_length as f32);
        let correlation: Vec<f32> = idft_buffer.iter()
            .map(|c| c.norm_sqr() * norm_factor)
            .collect();
        
        Ok(correlation)
    }
    
    /// Find peak in correlation window
    fn find_correlation_peak(
        &self,
        correlation: &[f32],
        window_start: usize,
        window_size: usize,
    ) -> Option<(usize, f32)> {
        let window_end = (window_start + window_size).min(correlation.len());
        
        let mut max_idx = window_start;
        let mut max_val = 0.0;
        
        for i in window_start..window_end {
            if correlation[i] > max_val {
                max_val = correlation[i];
                max_idx = i;
            }
        }
        
        if max_val > 0.0 {
            Some((max_idx, max_val))
        } else {
            None
        }
    }
    
    /// Calculate detection threshold based on noise level
    fn calculate_detection_threshold(&self, rssi: f32) -> f32 {
        // Simple threshold based on RSSI and configured target power
        // In practice, this would be more sophisticated
        let noise_factor = 3.0;  // 3x noise floor
        let min_threshold = 0.01;  // Minimum threshold
        
        (rssi * noise_factor).max(min_threshold)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_zc_sequence_generation() {
        let seq = PrachDetector::generate_zc_sequence(0, 839);
        assert_eq!(seq.len(), 839);
        
        // Check that all elements have unit magnitude
        for c in &seq {
            assert!((c.norm() - 1.0).abs() < 1e-6);
        }
    }
    
    #[test]
    fn test_prach_config_fdd() {
        let config = get_prach_config_fdd(0).unwrap();
        assert_eq!(config.format, PrachFormat::Format0);
        assert_eq!(config.x, 16);
        assert_eq!(config.y, vec![1]);
        assert_eq!(config.subframe_numbers, vec![9]);
    }
    
    #[test]
    fn test_prach_occasion_detection() {
        let rach_config = RachConfigCommon::default();
        let detector = PrachDetector::new(CellId(1), rach_config).unwrap();
        
        // Frame 1, slot 9 should be a PRACH occasion for config 0
        assert!(detector.is_prach_occasion(1, 9));
        
        // Frame 0, slot 9 should not be (y offset is 1)
        assert!(!detector.is_prach_occasion(0, 9));
        
        // Frame 1, slot 0 should not be (wrong subframe)
        assert!(!detector.is_prach_occasion(1, 0));
    }
}