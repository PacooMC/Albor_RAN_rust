//! OFDM Backend Selection
//! 
//! This module provides a unified interface for OFDM processing
//! with automatic selection between FlexRAN and software implementations.

use crate::LayerError;
use super::{CyclicPrefix, ResourceGrid};
use super::ofdm::{OfdmModulator, OfdmDemodulator};
use common::types::SubcarrierSpacing;
use num_complex::Complex32;
use tracing::info;

#[cfg(feature = "flexran")]
use super::flexran_adapter::{FlexranOfdmModulatorAdapter, OfdmBackend};

/// Unified OFDM modulator that can use either FlexRAN or software backend
pub enum UnifiedOfdmModulator {
    Software(OfdmModulator),
    #[cfg(feature = "flexran")]
    FlexRAN(FlexranOfdmModulatorAdapter),
}

impl UnifiedOfdmModulator {
    /// Create new OFDM modulator with automatic backend selection
    pub fn new(
        fft_size: usize,
        cp_type: CyclicPrefix,
        scs: SubcarrierSpacing,
    ) -> Result<Self, LayerError> {
        #[cfg(feature = "flexran")]
        {
            // Try FlexRAN first if feature is enabled
            if std::env::var("FLEXRAN_SDK_DIR").is_ok() {
                match FlexranOfdmModulatorAdapter::new(fft_size, cp_type, scs) {
                    Ok(adapter) => {
                        if adapter.is_accelerated() {
                            info!("Using FlexRAN hardware-accelerated OFDM modulator");
                            return Ok(UnifiedOfdmModulator::FlexRAN(adapter));
                        }
                    }
                    Err(e) => {
                        debug!("FlexRAN initialization failed: {}, falling back to software", e);
                    }
                }
            }
        }
        
        // Fallback to software implementation
        info!("Using software OFDM modulator (FFTW)");
        let modulator = OfdmModulator::new(fft_size, cp_type, scs)?;
        Ok(UnifiedOfdmModulator::Software(modulator))
    }
    
    /// Modulate one OFDM symbol
    pub fn modulate(&self, resource_grid: &ResourceGrid, symbol_index: u8) -> Vec<Complex32> {
        match self {
            UnifiedOfdmModulator::Software(mod_) => mod_.modulate(resource_grid, symbol_index),
            #[cfg(feature = "flexran")]
            UnifiedOfdmModulator::FlexRAN(adapter) => adapter.modulate(resource_grid, symbol_index),
        }
    }
    
    /// Modulate a complete slot
    pub fn modulate_slot(&self, resource_grid: &ResourceGrid) -> Vec<Complex32> {
        match self {
            UnifiedOfdmModulator::Software(mod_) => mod_.modulate_slot(resource_grid),
            #[cfg(feature = "flexran")]
            UnifiedOfdmModulator::FlexRAN(_adapter) => {
                // FlexRAN adapter doesn't have modulate_slot, so we implement it here
                let symbols_per_slot = match self.get_cp_type() {
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
        }
    }
    
    /// Get total samples per symbol including CP
    pub fn symbol_length(&self) -> usize {
        match self {
            UnifiedOfdmModulator::Software(mod_) => mod_.symbol_length(),
            #[cfg(feature = "flexran")]
            UnifiedOfdmModulator::FlexRAN(adapter) => adapter.symbol_length(),
        }
    }
    
    /// Set baseband gain in dB
    pub fn set_baseband_gain_db(&mut self, gain_db: f32) {
        match self {
            UnifiedOfdmModulator::Software(mod_) => mod_.set_baseband_gain_db(gain_db),
            #[cfg(feature = "flexran")]
            UnifiedOfdmModulator::FlexRAN(_adapter) => {
                // FlexRAN adapter manages gain internally
                debug!("Baseband gain request ignored for FlexRAN backend (managed internally)");
            }
        }
    }
    
    /// Configure bandwidth and update baseband gain accordingly
    pub fn configure_bandwidth(&mut self, bw_rb: usize, baseband_backoff_db: f32) {
        match self {
            UnifiedOfdmModulator::Software(mod_) => mod_.configure_bandwidth(bw_rb, baseband_backoff_db),
            #[cfg(feature = "flexran")]
            UnifiedOfdmModulator::FlexRAN(adapter) => adapter.configure_bandwidth(bw_rb, baseband_backoff_db),
        }
    }
    
    /// Apply phase compensation for carrier frequency offset
    pub fn apply_cfo_compensation(
        &self,
        samples: &mut [Complex32],
        cfo_hz: f32,
        sample_rate: f32,
    ) {
        match self {
            UnifiedOfdmModulator::Software(mod_) => mod_.apply_cfo_compensation(samples, cfo_hz, sample_rate),
            #[cfg(feature = "flexran")]
            UnifiedOfdmModulator::FlexRAN(_adapter) => {
                // Implement CFO compensation for FlexRAN
                use std::f32::consts::PI;
                let phase_increment = 2.0 * PI * cfo_hz / sample_rate;
                let mut phase: f32 = 0.0;
                
                for sample in samples {
                    let compensation = Complex32::new(phase.cos(), phase.sin());
                    *sample *= compensation;
                    phase += phase_increment;
                    
                    if phase > PI {
                        phase -= 2.0 * PI;
                    } else if phase < -PI {
                        phase += 2.0 * PI;
                    }
                }
            }
        }
    }
    
    /// Get backend type as string
    pub fn backend_type(&self) -> &'static str {
        match self {
            UnifiedOfdmModulator::Software(_) => "Software (FFTW)",
            #[cfg(feature = "flexran")]
            UnifiedOfdmModulator::FlexRAN(_) => "FlexRAN (Hardware Accelerated)",
        }
    }
    
    /// Check if using hardware acceleration
    pub fn is_accelerated(&self) -> bool {
        match self {
            UnifiedOfdmModulator::Software(_) => false,
            #[cfg(feature = "flexran")]
            UnifiedOfdmModulator::FlexRAN(adapter) => adapter.is_accelerated(),
        }
    }
    
    // Helper method to get CP type
    fn get_cp_type(&self) -> CyclicPrefix {
        // For now, we'll hard-code based on symbol count
        // In a real implementation, we'd expose this from both backends
        CyclicPrefix::Normal
    }
}

// Implement Clone manually
impl Clone for UnifiedOfdmModulator {
    fn clone(&self) -> Self {
        match self {
            UnifiedOfdmModulator::Software(mod_) => UnifiedOfdmModulator::Software(mod_.clone()),
            #[cfg(feature = "flexran")]
            UnifiedOfdmModulator::FlexRAN(adapter) => UnifiedOfdmModulator::FlexRAN(adapter.clone()),
        }
    }
}

/// Unified OFDM demodulator (currently software only)
pub type UnifiedOfdmDemodulator = OfdmDemodulator;

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_unified_modulator_creation() {
        let modulator = UnifiedOfdmModulator::new(
            2048,
            CyclicPrefix::Normal,
            SubcarrierSpacing::Scs15,
        );
        
        assert!(modulator.is_ok());
        let modulator = modulator.unwrap();
        
        // Should use software backend in test environment
        assert!(!modulator.is_accelerated());
        assert_eq!(modulator.backend_type(), "Software (FFTW)");
    }
}