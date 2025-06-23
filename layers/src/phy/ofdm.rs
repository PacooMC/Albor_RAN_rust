//! OFDM Modulation and Demodulation for 5G NR
//! 
//! Implements OFDM processing according to 3GPP TS 38.211

use crate::LayerError;
use common::types::SubcarrierSpacing;
use num_complex::Complex32;
use rustfft::{FftPlanner, Fft};
use std::sync::{Arc, Mutex};
use std::f32::consts::PI;
use tracing::debug;

use super::{CyclicPrefix, ResourceGrid};

/// OFDM modulator for downlink
#[derive(Clone)]
pub struct OfdmModulator {
    /// FFT size
    fft_size: usize,
    /// Cyclic prefix type
    cp_type: CyclicPrefix,
    /// Subcarrier spacing
    scs: SubcarrierSpacing,
    /// IFFT instance
    ifft: Arc<dyn Fft<f32>>,
    /// CP lengths for each symbol
    cp_lengths: Vec<usize>,
    /// Scratch buffer for FFT
    scratch: Arc<Mutex<Vec<Complex32>>>,
    /// Baseband gain in dB (backoff from full scale)
    baseband_gain_db: f32,
}

impl OfdmModulator {
    /// Create a new OFDM modulator
    pub fn new(
        fft_size: usize,
        cp_type: CyclicPrefix,
        scs: SubcarrierSpacing,
    ) -> Result<Self, LayerError> {
        let mut planner = FftPlanner::new();
        let ifft = planner.plan_fft_inverse(fft_size);
        
        // Calculate CP lengths for each symbol in slot
        let cp_lengths = calculate_cp_lengths(fft_size, cp_type, scs);
        
        // Create scratch buffer
        let scratch = Arc::new(Mutex::new(vec![Complex32::new(0.0, 0.0); ifft.get_inplace_scratch_len()]));
        
        Ok(Self {
            fft_size,
            cp_type,
            scs,
            ifft,
            cp_lengths,
            scratch,
            baseband_gain_db: -3.0, // 3 dB backoff to prevent clipping, similar to srsRAN
        })
    }
    
    /// Modulate one OFDM symbol
    pub fn modulate(&self, resource_grid: &ResourceGrid, symbol_index: u8) -> Vec<Complex32> {
        // Get frequency domain samples from resource grid
        let mut freq_samples = resource_grid.get_symbol(symbol_index);
        
        // Apply IFFT
        {
            let mut scratch = self.scratch.lock().unwrap();
            self.ifft.process_with_scratch(&mut freq_samples, &mut scratch);
        }
        
        // Scale by FFT size and apply baseband gain
        // srsRAN approach: normalize by 1/sqrt(N) and apply configured scale
        // This ensures proper power levels for ZMQ transmission
        let fft_scale = 1.0 / (self.fft_size as f32).sqrt();
        let baseband_gain = 10.0_f32.powf(self.baseband_gain_db / 20.0);
        let total_scale = fft_scale * baseband_gain;
        
        // Apply phase compensation if needed (srsRAN does this)
        // For now, we'll keep it simple without phase compensation
        
        for sample in &mut freq_samples {
            *sample *= total_scale;
        }
        
        // Log signal power after scaling
        let avg_power: f32 = freq_samples.iter().map(|s| s.norm_sqr()).sum::<f32>() / freq_samples.len() as f32;
        let peak_power: f32 = freq_samples.iter().map(|s| s.norm_sqr()).fold(0.0, f32::max);
        debug!("OFDM symbol {}: avg power={:.6} ({:.1} dB), peak power={:.6} ({:.1} dB), scale={:.6}", 
               symbol_index, avg_power, 10.0 * avg_power.log10(), peak_power, 10.0 * peak_power.log10(), total_scale);
        
        // Add cyclic prefix
        let cp_len = self.cp_lengths[symbol_index as usize % self.cp_lengths.len()];
        let mut output = Vec::with_capacity(self.fft_size + cp_len);
        
        // Copy last cp_len samples as CP
        output.extend_from_slice(&freq_samples[self.fft_size - cp_len..]);
        // Copy all samples
        output.extend_from_slice(&freq_samples);
        
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

/// OFDM demodulator for uplink
#[derive(Clone)]
pub struct OfdmDemodulator {
    /// FFT size
    fft_size: usize,
    /// Cyclic prefix type
    cp_type: CyclicPrefix,
    /// Subcarrier spacing
    scs: SubcarrierSpacing,
    /// FFT instance
    fft: Arc<dyn Fft<f32>>,
    /// CP lengths for each symbol
    cp_lengths: Vec<usize>,
    /// Scratch buffer for FFT
    scratch: Arc<Mutex<Vec<Complex32>>>,
}

impl OfdmDemodulator {
    /// Create a new OFDM demodulator
    pub fn new(
        fft_size: usize,
        cp_type: CyclicPrefix,
        scs: SubcarrierSpacing,
    ) -> Result<Self, LayerError> {
        let mut planner = FftPlanner::new();
        let fft = planner.plan_fft_forward(fft_size);
        
        // Calculate CP lengths for each symbol in slot
        let cp_lengths = calculate_cp_lengths(fft_size, cp_type, scs);
        
        // Create scratch buffer
        let scratch = Arc::new(Mutex::new(vec![Complex32::new(0.0, 0.0); fft.get_inplace_scratch_len()]));
        
        Ok(Self {
            fft_size,
            cp_type,
            scs,
            fft,
            cp_lengths,
            scratch,
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
        
        // Skip cyclic prefix and take FFT size samples
        let mut fft_input: Vec<Complex32> = time_samples[cp_len..].to_vec();
        
        // Apply FFT
        {
            let mut scratch = self.scratch.lock().unwrap();
            self.fft.process_with_scratch(&mut fft_input, &mut scratch);
        }
        
        // Scale by FFT size
        let scale = 1.0 / (self.fft_size as f32).sqrt();
        for sample in &mut fft_input {
            *sample *= scale;
        }
        
        Ok(fft_input)
    }
    
    /// Demodulate samples into frequency domain
    pub fn demodulate(&self, time_samples: &[Complex32]) -> Vec<Complex32> {
        // For simplicity, assume single symbol demodulation
        // In practice, this would handle multiple symbols
        if let Ok(freq_samples) = self.demodulate_symbol(time_samples, 0) {
            freq_samples
        } else {
            vec![Complex32::new(0.0, 0.0); self.fft_size]
        }
    }
    
    /// Estimate and compensate timing offset
    pub fn estimate_timing_offset(&self, samples: &[Complex32]) -> f32 {
        // Simple correlation-based timing estimation
        // In practice, use more sophisticated algorithms
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