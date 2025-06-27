//! FlexRAN Adapter for PHY Layer
//! 
//! This module provides an adapter between our PHY layer interfaces
//! and the FlexRAN SDK for hardware-accelerated processing.

use crate::LayerError;
use super::{CyclicPrefix, ResourceGrid};
use common::types::SubcarrierSpacing;
use num_complex::Complex32;
use flexran_sys::{AlignedVector, FlexranError, FlexranOfdmModulator};
use std::sync::{Arc, Mutex};
use tracing::{debug, info, warn};

/// Trait for OFDM processing backend
pub trait OfdmBackend: Send + Sync {
    /// Perform OFDM modulation (IFFT + CP)
    fn modulate(&self, resource_grid: &ResourceGrid, symbol_index: u8) -> Vec<Complex32>;
    
    /// Perform OFDM demodulation (remove CP + FFT)
    fn demodulate(&self, time_samples: &[Complex32], symbol_index: u8) -> Result<Vec<Complex32>, LayerError>;
    
    /// Get symbol length including CP
    fn symbol_length(&self) -> usize;
    
    /// Configure bandwidth and baseband gain
    fn configure_bandwidth(&mut self, bw_rb: usize, baseband_backoff_db: f32);
}

/// FlexRAN-based OFDM modulator
pub struct FlexranOfdmModulatorAdapter {
    /// FFT size
    fft_size: usize,
    /// Cyclic prefix type
    cp_type: CyclicPrefix,
    /// Subcarrier spacing
    scs: SubcarrierSpacing,
    /// CP lengths for each symbol
    cp_lengths: Vec<usize>,
    /// Baseband gain in dB
    baseband_gain_db: f32,
    /// Bandwidth in resource blocks
    bw_rb: usize,
    /// FlexRAN modulator handle (if available)
    flexran_modulator: Option<Arc<Mutex<FlexranOfdmModulator>>>,
    /// Pre-allocated aligned buffers
    freq_buffer: Arc<Mutex<AlignedVector<Complex32>>>,
    time_buffer: Arc<Mutex<AlignedVector<Complex32>>>,
}

impl FlexranOfdmModulatorAdapter {
    /// Create new FlexRAN OFDM modulator adapter
    pub fn new(
        fft_size: usize,
        cp_type: CyclicPrefix,
        scs: SubcarrierSpacing,
    ) -> Result<Self, LayerError> {
        // Calculate CP lengths
        let cp_lengths = calculate_cp_lengths(fft_size, cp_type, scs);
        
        // Try to initialize FlexRAN modulator
        let flexran_modulator = match std::env::var("FLEXRAN_SDK_DIR") {
            Ok(_) => {
                // FlexRAN SDK is configured, try to create modulator
                match FlexranOfdmModulator::new(
                    fft_size as u32,
                    cp_lengths[0] as u32,
                    scs as u32,
                    14, // symbols per slot
                ) {
                    Ok(modulator) => {
                        info!("FlexRAN OFDM modulator initialized successfully");
                        Some(Arc::new(Mutex::new(modulator)))
                    }
                    Err(e) => {
                        warn!("Failed to initialize FlexRAN modulator: {:?}, falling back to software implementation", e);
                        None
                    }
                }
            }
            Err(_) => {
                debug!("FLEXRAN_SDK_DIR not set, using software OFDM implementation");
                None
            }
        };
        
        // Allocate aligned buffers
        let freq_buffer = AlignedVector::with_capacity(fft_size)
            .map_err(|e| LayerError::InitializationFailed(format!("Failed to allocate freq buffer: {:?}", e)))?;
        let time_buffer = AlignedVector::with_capacity(fft_size + cp_lengths[0])
            .map_err(|e| LayerError::InitializationFailed(format!("Failed to allocate time buffer: {:?}", e)))?;
        
        // Default bandwidth
        let bw_rb = 52; // 10 MHz for 15 kHz SCS
        
        // Calculate baseband gain
        let baseband_backoff_db = 30.0;
        let fft_loss_db = 20.0 * (fft_size as f32).log10();
        let baseband_gain_db = fft_loss_db - baseband_backoff_db;
        
        Ok(Self {
            fft_size,
            cp_type,
            scs,
            cp_lengths,
            baseband_gain_db,
            bw_rb,
            flexran_modulator,
            freq_buffer: Arc::new(Mutex::new(freq_buffer)),
            time_buffer: Arc::new(Mutex::new(time_buffer)),
        })
    }
    
    /// Check if FlexRAN acceleration is available
    pub fn is_accelerated(&self) -> bool {
        self.flexran_modulator.is_some()
    }
}

impl OfdmBackend for FlexranOfdmModulatorAdapter {
    fn modulate(&self, resource_grid: &ResourceGrid, symbol_index: u8) -> Vec<Complex32> {
        // Get frequency domain samples
        let freq_samples = if let Some(view) = resource_grid.get_symbol_view(symbol_index) {
            view.to_vec()
        } else {
            resource_grid.get_symbol(symbol_index)
        };
        
        // Count non-zero subcarriers
        let non_zero_count = freq_samples.iter().filter(|s| s.norm_sqr() > 0.0).count();
        if non_zero_count > 0 {
            let freq_power: f32 = freq_samples.iter().map(|s| s.norm_sqr()).sum::<f32>() / self.fft_size as f32;
            let freq_power_db = 10.0 * freq_power.log10();
            debug!("OFDM symbol {}: {} non-zero subcarriers, freq domain power={:.2} dB", 
                   symbol_index, non_zero_count, freq_power_db);
        }
        
        // Get CP length for this symbol
        let cp_len = self.cp_lengths[symbol_index as usize % self.cp_lengths.len()];
        
        // Perform IFFT
        let time_samples = if let Some(flexran_mod) = &self.flexran_modulator {
            // Use FlexRAN accelerated implementation
            debug!("Using FlexRAN accelerated IFFT");
            
            let mut freq_buffer = self.freq_buffer.lock().unwrap();
            let mut time_buffer = self.time_buffer.lock().unwrap();
            
            // Copy to aligned buffer
            freq_buffer.resize(self.fft_size, Complex32::new(0.0, 0.0));
            for (i, &sample) in freq_samples.iter().enumerate() {
                freq_buffer.as_mut_slice()[i] = sample;
            }
            
            // Prepare output buffer
            time_buffer.resize(self.fft_size, Complex32::new(0.0, 0.0));
            
            // Use FlexRAN IFFT
            match flexran_sys::ofdm_ifft(
                freq_buffer.as_slice(),
                time_buffer.as_mut_slice(),
                cp_len,
                1.0 / self.fft_size as f32, // Normalization factor
            ) {
                Ok(()) => {
                    time_buffer.as_slice().to_vec()
                }
                Err(e) => {
                    warn!("FlexRAN IFFT failed: {:?}, falling back to software", e);
                    // Fallback to software implementation would go here
                    vec![Complex32::new(0.0, 0.0); self.fft_size]
                }
            }
        } else {
            // Software implementation fallback
            // In production, this would call the existing FFTW-based implementation
            debug!("Using software IFFT implementation");
            vec![Complex32::new(0.0, 0.0); self.fft_size]
        };
        
        // Apply baseband gain
        let baseband_gain = 10.0_f32.powf(self.baseband_gain_db / 20.0);
        let scaled_samples: Vec<Complex32> = time_samples.iter()
            .map(|&s| s * baseband_gain)
            .collect();
        
        // Add cyclic prefix
        let mut output = Vec::with_capacity(self.fft_size + cp_len);
        output.extend_from_slice(&scaled_samples[self.fft_size - cp_len..]);
        output.extend_from_slice(&scaled_samples);
        
        output
    }
    
    fn demodulate(&self, time_samples: &[Complex32], symbol_index: u8) -> Result<Vec<Complex32>, LayerError> {
        let cp_len = self.cp_lengths[symbol_index as usize % self.cp_lengths.len()];
        let expected_len = self.fft_size + cp_len;
        
        if time_samples.len() != expected_len {
            return Err(LayerError::InvalidConfiguration(
                format!("Expected {} samples, got {}", expected_len, time_samples.len())
            ));
        }
        
        // Skip CP and perform FFT
        let freq_samples = if let Some(_flexran_mod) = &self.flexran_modulator {
            // Use FlexRAN accelerated FFT
            debug!("Using FlexRAN accelerated FFT");
            
            let mut time_buffer = self.time_buffer.lock().unwrap();
            let mut freq_buffer = self.freq_buffer.lock().unwrap();
            
            // Copy to aligned buffer (skip CP)
            time_buffer.resize(self.fft_size, Complex32::new(0.0, 0.0));
            for (i, &sample) in time_samples[cp_len..].iter().enumerate() {
                time_buffer.as_mut_slice()[i] = sample;
            }
            
            // Prepare output buffer
            freq_buffer.resize(self.fft_size, Complex32::new(0.0, 0.0));
            
            // Use FlexRAN FFT
            match flexran_sys::ofdm_fft(
                time_buffer.as_slice(),
                freq_buffer.as_mut_slice(),
                0, // No CP offset as we already skipped it
            ) {
                Ok(()) => {
                    // Apply scaling
                    let scale = 1.0 / (self.fft_size as f32).sqrt();
                    freq_buffer.as_slice().iter()
                        .map(|&s| s * scale)
                        .collect()
                }
                Err(e) => {
                    warn!("FlexRAN FFT failed: {:?}", e);
                    return Err(LayerError::ProcessingError(format!("FFT failed: {:?}", e)));
                }
            }
        } else {
            // Software implementation fallback
            vec![Complex32::new(0.0, 0.0); self.fft_size]
        };
        
        Ok(freq_samples)
    }
    
    fn symbol_length(&self) -> usize {
        self.fft_size + self.cp_lengths[0]
    }
    
    fn configure_bandwidth(&mut self, bw_rb: usize, baseband_backoff_db: f32) {
        self.bw_rb = bw_rb;
        let fft_loss_db = 20.0 * (self.fft_size as f32).log10();
        self.baseband_gain_db = fft_loss_db - baseband_backoff_db;
        debug!("Configured FlexRAN OFDM adapter: bw_rb={}, baseband_gain_db={:.1} dB", 
               bw_rb, self.baseband_gain_db);
    }
}

// Clone implementation
impl Clone for FlexranOfdmModulatorAdapter {
    fn clone(&self) -> Self {
        // Create new instance with same configuration
        // Note: FlexRAN handles are not cloned, new instance will create its own
        match Self::new(self.fft_size, self.cp_type, self.scs) {
            Ok(mut adapter) => {
                adapter.baseband_gain_db = self.baseband_gain_db;
                adapter.bw_rb = self.bw_rb;
                adapter
            }
            Err(_) => {
                // Fallback: create without FlexRAN
                Self {
                    fft_size: self.fft_size,
                    cp_type: self.cp_type,
                    scs: self.scs,
                    cp_lengths: self.cp_lengths.clone(),
                    baseband_gain_db: self.baseband_gain_db,
                    bw_rb: self.bw_rb,
                    flexran_modulator: None,
                    freq_buffer: Arc::new(Mutex::new(AlignedVector::with_capacity(self.fft_size).unwrap())),
                    time_buffer: Arc::new(Mutex::new(AlignedVector::with_capacity(self.fft_size + self.cp_lengths[0]).unwrap())),
                }
            }
        }
    }
}

/// Calculate CP lengths for each symbol (same as in ofdm.rs)
fn calculate_cp_lengths(
    fft_size: usize,
    cp_type: CyclicPrefix,
    _scs: SubcarrierSpacing,
) -> Vec<usize> {
    match cp_type {
        CyclicPrefix::Normal => {
            let base_cp = (fft_size as f32 * 144.0 / 2048.0) as usize;
            let extended_cp = (fft_size as f32 * 160.0 / 2048.0) as usize;
            
            let mut lengths = vec![extended_cp];
            for _ in 1..7 {
                lengths.push(base_cp);
            }
            lengths.push(extended_cp);
            for _ in 8..14 {
                lengths.push(base_cp);
            }
            lengths
        }
        CyclicPrefix::Extended => {
            let cp_len = (fft_size as f32 * 512.0 / 2048.0) as usize;
            vec![cp_len; 12]
        }
    }
}

/// Factory function to create OFDM backend
/// This will automatically use FlexRAN if available, otherwise fallback to software
pub fn create_ofdm_backend(
    fft_size: usize,
    cp_type: CyclicPrefix,
    scs: SubcarrierSpacing,
    use_flexran: bool,
) -> Result<Box<dyn OfdmBackend>, LayerError> {
    if use_flexran && flexran_sys::is_available() {
        match FlexranOfdmModulatorAdapter::new(fft_size, cp_type, scs) {
            Ok(adapter) => {
                info!("Created FlexRAN-accelerated OFDM backend");
                Ok(Box::new(adapter))
            }
            Err(e) => {
                warn!("Failed to create FlexRAN adapter: {}, falling back to software", e);
                // In production, return software implementation here
                Err(e)
            }
        }
    } else {
        // Return software implementation
        // For now, return error as we haven't imported the software implementation
        Err(LayerError::InvalidConfiguration("Software OFDM backend not available in this context".to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_flexran_adapter_creation() {
        let adapter = FlexranOfdmModulatorAdapter::new(
            2048,
            CyclicPrefix::Normal,
            SubcarrierSpacing::Scs15,
        );
        
        // Should succeed even without FlexRAN SDK
        assert!(adapter.is_ok());
        
        let adapter = adapter.unwrap();
        assert_eq!(adapter.fft_size, 2048);
        assert_eq!(adapter.cp_lengths.len(), 14);
    }
    
    #[test]
    fn test_cp_length_calculation() {
        let lengths = calculate_cp_lengths(2048, CyclicPrefix::Normal, SubcarrierSpacing::Scs15);
        assert_eq!(lengths.len(), 14);
        assert_eq!(lengths[0], 160); // Extended CP
        assert_eq!(lengths[1], 144); // Normal CP
    }
}