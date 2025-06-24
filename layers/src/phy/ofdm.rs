//! OFDM Modulation and Demodulation for 5G NR
//! 
//! Implements OFDM processing according to 3GPP TS 38.211
//! Using FFTW for high-performance FFT operations

use crate::LayerError;
use common::types::SubcarrierSpacing;
use num_complex::Complex32;
use fftw::array::AlignedVec;
use fftw::plan::{C2CPlan32, C2CPlan};
use fftw::types::{Flag, Sign};
use std::sync::{Arc, Mutex};
use std::f32::consts::PI;
use tracing::debug;

use super::{CyclicPrefix, ResourceGrid};

/// OFDM modulator for downlink
pub struct OfdmModulator {
    /// FFT size
    fft_size: usize,
    /// Cyclic prefix type
    cp_type: CyclicPrefix,
    /// Subcarrier spacing
    scs: SubcarrierSpacing,
    /// IFFT plan (pre-computed for performance)
    ifft_plan: Arc<Mutex<C2CPlan32>>,
    /// Pre-allocated input buffer for IFFT
    ifft_input: Arc<Mutex<AlignedVec<num_complex::Complex<f32>>>>,
    /// Pre-allocated output buffer for IFFT
    ifft_output: Arc<Mutex<AlignedVec<num_complex::Complex<f32>>>>,
    /// CP lengths for each symbol
    cp_lengths: Vec<usize>,
    /// Baseband gain in dB (backoff from full scale)
    baseband_gain_db: f32,
    /// Bandwidth in resource blocks
    bw_rb: usize,
    /// Apply FFT normalization (1/sqrt(N))
    apply_fft_normalization: bool,
}

impl OfdmModulator {
    /// Create a new OFDM modulator
    pub fn new(
        fft_size: usize,
        cp_type: CyclicPrefix,
        scs: SubcarrierSpacing,
    ) -> Result<Self, LayerError> {
        // Create FFTW plan for inverse FFT
        // Use MEASURE flag for optimal performance (takes more time to plan but faster execution)
        let ifft_input = AlignedVec::new(fft_size);
        let ifft_output = AlignedVec::new(fft_size);
        
        let ifft_plan = C2CPlan32::aligned(
            &[fft_size],
            Sign::Backward,
            Flag::MEASURE | Flag::DESTROYINPUT,
        ).map_err(|e| LayerError::InvalidConfiguration(format!("Failed to create IFFT plan: {:?}", e)))?;
        
        // Calculate CP lengths for each symbol in slot
        let cp_lengths = calculate_cp_lengths(fft_size, cp_type, scs);
        
        // Default bandwidth - will be updated by configure_bandwidth
        let bw_rb = 52; // Default 10 MHz for 15 kHz SCS
        
        // Calculate baseband gain following srsRAN approach
        // Note: We need stronger signal for ZMQ interface to work properly
        let baseband_backoff_db = 6.0;  // Reduced backoff for stronger signal
        let power_db = 10.0 * (bw_rb as f32 * 12.0).log10();
        let baseband_gain_db = -power_db - baseband_backoff_db;
        
        Ok(Self {
            fft_size,
            cp_type,
            scs,
            ifft_plan: Arc::new(Mutex::new(ifft_plan)),
            ifft_input: Arc::new(Mutex::new(ifft_input)),
            ifft_output: Arc::new(Mutex::new(ifft_output)),
            cp_lengths,
            baseband_gain_db,
            bw_rb,
            apply_fft_normalization: false, // srsRAN doesn't apply 1/sqrt(N)
        })
    }
    
    /// Modulate one OFDM symbol
    pub fn modulate(&self, resource_grid: &ResourceGrid, symbol_index: u8) -> Vec<Complex32> {
        // Get frequency domain samples
        let freq_samples = if let Some(view) = resource_grid.get_symbol_view(symbol_index) {
            view.to_vec()
        } else {
            resource_grid.get_symbol(symbol_index)
        };
        
        // Perform IFFT using FFTW
        let time_samples = {
            let mut ifft_plan = self.ifft_plan.lock().unwrap();
            let mut ifft_input = self.ifft_input.lock().unwrap();
            let mut ifft_output = self.ifft_output.lock().unwrap();
            
            // Copy input data to aligned buffer
            for (i, &sample) in freq_samples.iter().enumerate() {
                ifft_input[i] = num_complex::Complex::new(sample.re, sample.im);
            }
            
            // Execute IFFT
            ifft_plan.c2c(&mut ifft_input, &mut ifft_output)
                .map_err(|e| LayerError::ProcessingError(format!("IFFT failed: {:?}", e)))
                .unwrap();
            
            // Convert back to Complex32
            ifft_output.iter()
                .map(|&c| Complex32::new(c.re, c.im))
                .collect::<Vec<_>>()
        };
        
        // Scale by FFT size and apply baseband gain
        let fft_scale = if self.apply_fft_normalization {
            1.0 / (self.fft_size as f32).sqrt()
        } else {
            1.0
        };
        let baseband_gain = 10.0_f32.powf(self.baseband_gain_db / 20.0);
        let total_scale = fft_scale * baseband_gain;
        
        let scaled_samples: Vec<Complex32> = time_samples.iter()
            .map(|&s| s * total_scale)
            .collect();
        
        // Add cyclic prefix
        let cp_len = self.cp_lengths[symbol_index as usize % self.cp_lengths.len()];
        let mut output = Vec::with_capacity(self.fft_size + cp_len);
        
        // Copy last cp_len samples as CP
        output.extend_from_slice(&scaled_samples[self.fft_size - cp_len..]);
        // Copy all samples
        output.extend_from_slice(&scaled_samples);
        
        output
    }
    
    /// Modulate a complete slot
    pub fn modulate_slot(&self, resource_grid: &ResourceGrid) -> Vec<Complex32> {
        let symbols_per_slot = match self.cp_type {
            CyclicPrefix::Normal => 14,
            CyclicPrefix::Extended => 12,
        };
        
        let mut output = Vec::new();
        
        for symbol in 0..symbols_per_slot {
            let symbol_samples = self.modulate(resource_grid, symbol);
            output.extend(symbol_samples);
        }
        
        output
    }
    
    /// Get total samples per symbol including CP
    pub fn symbol_length(&self) -> usize {
        self.fft_size + self.cp_lengths[0]
    }
    
    /// Set baseband gain in dB
    pub fn set_baseband_gain_db(&mut self, gain_db: f32) {
        self.baseband_gain_db = gain_db;
    }
    
    /// Configure bandwidth and update baseband gain accordingly
    pub fn configure_bandwidth(&mut self, bw_rb: usize, baseband_backoff_db: f32) {
        self.bw_rb = bw_rb;
        // Update baseband gain following srsRAN formula
        // Using reduced backoff for stronger signal with ZMQ
        let actual_backoff = baseband_backoff_db.min(6.0);  // Cap at 6 dB for ZMQ
        let power_db = 10.0 * (bw_rb as f32 * 12.0).log10();
        self.baseband_gain_db = -power_db - actual_backoff;
        debug!("Configured OFDM modulator: bw_rb={}, baseband_gain_db={:.1} (backoff={:.1}dB)", 
               bw_rb, self.baseband_gain_db, actual_backoff);
    }
    
    /// Apply phase compensation for carrier frequency offset
    pub fn apply_cfo_compensation(
        &self,
        samples: &mut [Complex32],
        cfo_hz: f32,
        sample_rate: f32,
    ) {
        let phase_increment = 2.0 * PI * cfo_hz / sample_rate;
        let mut phase: f32 = 0.0;
        
        for sample in samples {
            let compensation = Complex32::new(phase.cos(), phase.sin());
            *sample *= compensation;
            phase += phase_increment;
            
            // Wrap phase to [-π, π]
            if phase > PI {
                phase -= 2.0 * PI;
            } else if phase < -PI {
                phase += 2.0 * PI;
            }
        }
    }
}

// Implement Clone manually since FFTW plans don't implement Clone
impl Clone for OfdmModulator {
    fn clone(&self) -> Self {
        // Create new FFTW plan for the clone
        let mut ifft_input = AlignedVec::new(self.fft_size);
        let mut ifft_output = AlignedVec::new(self.fft_size);
        
        let ifft_plan = C2CPlan32::aligned(
            &[self.fft_size],
            Sign::Backward,
            Flag::MEASURE | Flag::DESTROYINPUT,
        ).expect("Failed to create IFFT plan for clone");
        
        Self {
            fft_size: self.fft_size,
            cp_type: self.cp_type,
            scs: self.scs,
            ifft_plan: Arc::new(Mutex::new(ifft_plan)),
            ifft_input: Arc::new(Mutex::new(ifft_input)),
            ifft_output: Arc::new(Mutex::new(ifft_output)),
            cp_lengths: self.cp_lengths.clone(),
            baseband_gain_db: self.baseband_gain_db,
            bw_rb: self.bw_rb,
            apply_fft_normalization: self.apply_fft_normalization,
        }
    }
}

/// OFDM demodulator for uplink
pub struct OfdmDemodulator {
    /// FFT size
    fft_size: usize,
    /// Cyclic prefix type
    cp_type: CyclicPrefix,
    /// Subcarrier spacing
    scs: SubcarrierSpacing,
    /// FFT plan (pre-computed for performance)
    fft_plan: Arc<Mutex<C2CPlan32>>,
    /// Pre-allocated input buffer for FFT
    fft_input: Arc<Mutex<AlignedVec<num_complex::Complex<f32>>>>,
    /// Pre-allocated output buffer for FFT
    fft_output: Arc<Mutex<AlignedVec<num_complex::Complex<f32>>>>,
    /// CP lengths for each symbol
    cp_lengths: Vec<usize>,
}

impl OfdmDemodulator {
    /// Create a new OFDM demodulator
    pub fn new(
        fft_size: usize,
        cp_type: CyclicPrefix,
        scs: SubcarrierSpacing,
    ) -> Result<Self, LayerError> {
        // Create FFTW plan for forward FFT
        let fft_input = AlignedVec::new(fft_size);
        let fft_output = AlignedVec::new(fft_size);
        
        let fft_plan = C2CPlan32::aligned(
            &[fft_size],
            Sign::Forward,
            Flag::MEASURE | Flag::DESTROYINPUT,
        ).map_err(|e| LayerError::InvalidConfiguration(format!("Failed to create FFT plan: {:?}", e)))?;
        
        // Calculate CP lengths for each symbol in slot
        let cp_lengths = calculate_cp_lengths(fft_size, cp_type, scs);
        
        Ok(Self {
            fft_size,
            cp_type,
            scs,
            fft_plan: Arc::new(Mutex::new(fft_plan)),
            fft_input: Arc::new(Mutex::new(fft_input)),
            fft_output: Arc::new(Mutex::new(fft_output)),
            cp_lengths,
        })
    }
    
    /// Demodulate one OFDM symbol
    pub fn demodulate_symbol(
        &self,
        time_samples: &[Complex32],
        symbol_index: u8,
    ) -> Result<Vec<Complex32>, LayerError> {
        let cp_len = self.cp_lengths[symbol_index as usize % self.cp_lengths.len()];
        let expected_len = self.fft_size + cp_len;
        
        if time_samples.len() != expected_len {
            return Err(LayerError::InvalidConfiguration(
                format!("Expected {} samples, got {}", expected_len, time_samples.len())
            ));
        }
        
        // Skip cyclic prefix and perform FFT
        let freq_samples = {
            let mut fft_plan = self.fft_plan.lock().unwrap();
            let mut fft_input = self.fft_input.lock().unwrap();
            let mut fft_output = self.fft_output.lock().unwrap();
            
            // Copy input data (skip CP)
            for (i, &sample) in time_samples[cp_len..].iter().enumerate() {
                fft_input[i] = num_complex::Complex::new(sample.re, sample.im);
            }
            
            // Execute FFT
            fft_plan.c2c(&mut fft_input, &mut fft_output)
                .map_err(|e| LayerError::ProcessingError(format!("FFT failed: {:?}", e)))?;
            
            // Convert back to Complex32 and scale
            let scale = 1.0 / (self.fft_size as f32).sqrt();
            fft_output.iter()
                .map(|&c| Complex32::new(c.re * scale, c.im * scale))
                .collect::<Vec<_>>()
        };
        
        Ok(freq_samples)
    }
    
    /// Demodulate samples into frequency domain
    pub fn demodulate(&self, time_samples: &[Complex32]) -> Vec<Complex32> {
        // For simplicity, assume single symbol demodulation
        if let Ok(freq_samples) = self.demodulate_symbol(time_samples, 0) {
            freq_samples
        } else {
            vec![Complex32::new(0.0, 0.0); self.fft_size]
        }
    }
    
    /// Estimate and compensate timing offset
    pub fn estimate_timing_offset(&self, samples: &[Complex32]) -> f32 {
        // Simple correlation-based timing estimation
        let cp_len = self.cp_lengths[0];
        
        if samples.len() < self.fft_size + cp_len {
            return 0.0;
        }
        
        let mut correlation = Complex32::new(0.0, 0.0);
        let mut power = 0.0;
        
        // Correlate CP with end of symbol
        for i in 0..cp_len {
            correlation += samples[i] * samples[i + self.fft_size].conj();
            power += samples[i].norm_sqr() + samples[i + self.fft_size].norm_sqr();
        }
        
        // Timing metric
        let metric = correlation.norm() / (power / 2.0);
        
        // Convert to sample offset (simplified)
        metric * cp_len as f32
    }
    
    /// Estimate carrier frequency offset
    pub fn estimate_cfo(&self, samples: &[Complex32]) -> f32 {
        let cp_len = self.cp_lengths[0];
        
        if samples.len() < self.fft_size + cp_len {
            return 0.0;
        }
        
        let mut phase_sum = 0.0;
        let mut count = 0;
        
        // Use CP correlation for CFO estimation
        for i in 0..cp_len {
            let correlation = samples[i] * samples[i + self.fft_size].conj();
            if correlation.norm() > 0.0 {
                phase_sum += correlation.arg();
                count += 1;
            }
        }
        
        if count > 0 {
            let avg_phase = phase_sum / count as f32;
            // Convert phase to frequency offset
            let sample_rate = calculate_sample_rate(self.fft_size, self.scs);
            avg_phase * sample_rate / (2.0 * PI * self.fft_size as f32)
        } else {
            0.0
        }
    }
}

// Implement Clone manually since FFTW plans don't implement Clone
impl Clone for OfdmDemodulator {
    fn clone(&self) -> Self {
        // Create new FFTW plan for the clone
        let mut fft_input = AlignedVec::new(self.fft_size);
        let mut fft_output = AlignedVec::new(self.fft_size);
        
        let fft_plan = C2CPlan32::aligned(
            &[self.fft_size],
            Sign::Forward,
            Flag::MEASURE | Flag::DESTROYINPUT,
        ).expect("Failed to create FFT plan for clone");
        
        Self {
            fft_size: self.fft_size,
            cp_type: self.cp_type,
            scs: self.scs,
            fft_plan: Arc::new(Mutex::new(fft_plan)),
            fft_input: Arc::new(Mutex::new(fft_input)),
            fft_output: Arc::new(Mutex::new(fft_output)),
            cp_lengths: self.cp_lengths.clone(),
        }
    }
}

/// Calculate CP lengths for each symbol
fn calculate_cp_lengths(
    fft_size: usize,
    cp_type: CyclicPrefix,
    _scs: SubcarrierSpacing,
) -> Vec<usize> {
    match cp_type {
        CyclicPrefix::Normal => {
            // Normal CP: first symbol in slot has longer CP
            let base_cp = (fft_size as f32 * 144.0 / 2048.0) as usize;
            let extended_cp = (fft_size as f32 * 160.0 / 2048.0) as usize;
            
            let mut lengths = vec![extended_cp]; // First symbol
            for _ in 1..7 {
                lengths.push(base_cp);
            }
            lengths.push(extended_cp); // 8th symbol
            for _ in 8..14 {
                lengths.push(base_cp);
            }
            lengths
        }
        CyclicPrefix::Extended => {
            // Extended CP: all symbols have same CP length
            let cp_len = (fft_size as f32 * 512.0 / 2048.0) as usize;
            vec![cp_len; 12]
        }
    }
}

/// Calculate sample rate from FFT size and SCS
fn calculate_sample_rate(fft_size: usize, scs: SubcarrierSpacing) -> f32 {
    let scs_hz = match scs {
        SubcarrierSpacing::Scs15 => 15_000.0,
        SubcarrierSpacing::Scs30 => 30_000.0,
        SubcarrierSpacing::Scs60 => 60_000.0,
        SubcarrierSpacing::Scs120 => 120_000.0,
        SubcarrierSpacing::Scs240 => 240_000.0,
    };
    
    fft_size as f32 * scs_hz
}

/// OFDM symbol timing information
#[derive(Debug, Clone)]
pub struct OfdmSymbolTiming {
    /// Symbol start sample
    pub start_sample: usize,
    /// Symbol duration in samples (including CP)
    pub duration: usize,
    /// CP length in samples
    pub cp_length: usize,
}

/// Calculate OFDM symbol timing for a slot
pub fn calculate_slot_timing(
    fft_size: usize,
    cp_type: CyclicPrefix,
    scs: SubcarrierSpacing,
) -> Vec<OfdmSymbolTiming> {
    let cp_lengths = calculate_cp_lengths(fft_size, cp_type, scs);
    let mut timings = Vec::new();
    let mut start = 0;
    
    for (_symbol_idx, &cp_len) in cp_lengths.iter().enumerate() {
        let duration = fft_size + cp_len;
        timings.push(OfdmSymbolTiming {
            start_sample: start,
            duration,
            cp_length: cp_len,
        });
        start += duration;
    }
    
    timings
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_ofdm_modulator() {
        let modulator = OfdmModulator::new(
            2048,
            CyclicPrefix::Normal,
            SubcarrierSpacing::Scs15,
        ).unwrap();
        
        assert_eq!(modulator.fft_size, 2048);
        assert_eq!(modulator.cp_lengths.len(), 14);
    }
    
    #[test]
    fn test_cp_lengths() {
        let lengths = calculate_cp_lengths(2048, CyclicPrefix::Normal, SubcarrierSpacing::Scs15);
        assert_eq!(lengths.len(), 14);
        assert_eq!(lengths[0], 160); // Extended CP for first symbol
        assert_eq!(lengths[1], 144); // Normal CP
    }
    
    #[test]
    fn test_sample_rate() {
        let rate = calculate_sample_rate(2048, SubcarrierSpacing::Scs15);
        assert_eq!(rate, 30_720_000.0); // 30.72 MHz
        
        let rate = calculate_sample_rate(4096, SubcarrierSpacing::Scs30);
        assert_eq!(rate, 122_880_000.0); // 122.88 MHz
    }
}