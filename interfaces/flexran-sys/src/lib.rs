//! FlexRAN FFI System Library
//! 
//! This crate provides low-level FFI bindings to the Intel FlexRAN SDK
//! for accelerated 5G PHY layer processing.
//! 
//! # Features
//! - AVX512 optimized FFT/IFFT operations
//! - LDPC encoding/decoding
//! - Channel estimation
//! - Memory alignment utilities
//! 
//! # Safety
//! This crate contains unsafe FFI bindings. Users should prefer the safe
//! wrapper functions and types provided by the higher-level flexran adapter.

#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]

pub mod ffi;

// Re-export commonly used types
pub use ffi::{
    AlignedVector, FftComplex, FlexranError, FlexranFftConfig, FlexranOfdmConfig,
    FlexranPhyConfig, FLEXRAN_ALIGNMENT, aligned_alloc, aligned_free,
};

use num_complex::Complex32;
use std::ptr;

/// Safe wrapper for FlexRAN OFDM IFFT operation
pub fn ofdm_ifft(
    freq_domain: &[Complex32],
    time_domain: &mut [Complex32],
    cp_len: usize,
    scaling_factor: f32,
) -> Result<(), FlexranError> {
    if freq_domain.len() != time_domain.len() {
        return Err(FlexranError::InvalidParameter);
    }
    
    let fft_size = freq_domain.len() as u32;
    
    // Convert to FFI types
    let freq_ffi: Vec<FftComplex> = freq_domain.iter().map(|&c| c.into()).collect();
    let mut time_ffi: Vec<FftComplex> = vec![FftComplex { re: 0.0, im: 0.0 }; time_domain.len()];
    
    let result = unsafe {
        ffi::flexran_ofdm_ifft(
            freq_ffi.as_ptr(),
            time_ffi.as_mut_ptr(),
            fft_size,
            cp_len as u32,
            scaling_factor,
        )
    };
    
    if result == 0 {
        // Convert back to Complex32
        for (i, &ffi_val) in time_ffi.iter().enumerate() {
            time_domain[i] = ffi_val.into();
        }
        Ok(())
    } else {
        Err(FlexranError::ProcessingFailed)
    }
}

/// Safe wrapper for FlexRAN OFDM FFT operation
pub fn ofdm_fft(
    time_domain: &[Complex32],
    freq_domain: &mut [Complex32],
    cp_offset: usize,
) -> Result<(), FlexranError> {
    if time_domain.len() != freq_domain.len() + cp_offset {
        return Err(FlexranError::InvalidParameter);
    }
    
    let fft_size = freq_domain.len() as u32;
    
    // Convert to FFI types
    let time_ffi: Vec<FftComplex> = time_domain.iter().map(|&c| c.into()).collect();
    let mut freq_ffi: Vec<FftComplex> = vec![FftComplex { re: 0.0, im: 0.0 }; freq_domain.len()];
    
    let result = unsafe {
        ffi::flexran_ofdm_fft(
            time_ffi.as_ptr(),
            freq_ffi.as_mut_ptr(),
            fft_size,
            cp_offset as u32,
        )
    };
    
    if result == 0 {
        // Convert back to Complex32
        for (i, &ffi_val) in freq_ffi.iter().enumerate() {
            freq_domain[i] = ffi_val.into();
        }
        Ok(())
    } else {
        Err(FlexranError::ProcessingFailed)
    }
}

/// OFDM modulator handle using FlexRAN
pub struct FlexranOfdmModulator {
    handle: *mut std::ffi::c_void,
    config: FlexranOfdmConfig,
}

impl FlexranOfdmModulator {
    /// Create new OFDM modulator
    pub fn new(fft_size: u32, cp_len: u32, scs: u32, num_symbols: u32) -> Result<Self, FlexranError> {
        let config = FlexranOfdmConfig {
            fft_size,
            cp_len,
            subcarrier_spacing: scs,
            num_symbols,
        };
        
        let mut handle = ptr::null_mut();
        let result = unsafe { ffi::flexran_ofdm_modulator_init(&config, &mut handle) };
        
        if result == 0 && !handle.is_null() {
            Ok(FlexranOfdmModulator { handle, config })
        } else {
            Err(FlexranError::NotInitialized)
        }
    }
    
    /// Process OFDM symbols
    pub fn process(
        &mut self,
        freq_grid: &[Complex32],
        time_samples: &mut [Complex32],
        num_symbols: u32,
    ) -> Result<(), FlexranError> {
        // Convert to FFI types
        let freq_ffi: Vec<FftComplex> = freq_grid.iter().map(|&c| c.into()).collect();
        let mut time_ffi: Vec<FftComplex> = vec![FftComplex { re: 0.0, im: 0.0 }; time_samples.len()];
        
        let result = unsafe {
            ffi::flexran_ofdm_modulator_process(
                self.handle,
                freq_ffi.as_ptr(),
                time_ffi.as_mut_ptr(),
                num_symbols,
            )
        };
        
        if result == 0 {
            // Convert back
            for (i, &ffi_val) in time_ffi.iter().enumerate() {
                if i < time_samples.len() {
                    time_samples[i] = ffi_val.into();
                }
            }
            Ok(())
        } else {
            Err(FlexranError::ProcessingFailed)
        }
    }
}

impl Drop for FlexranOfdmModulator {
    fn drop(&mut self) {
        if !self.handle.is_null() {
            unsafe {
                ffi::flexran_ofdm_modulator_destroy(self.handle);
            }
        }
    }
}

// Safety: FlexranOfdmModulator owns its handle
unsafe impl Send for FlexranOfdmModulator {}
unsafe impl Sync for FlexranOfdmModulator {}

/// Check if FlexRAN is available and properly configured
pub fn is_available() -> bool {
    // Try to allocate a small aligned buffer as a test
    match aligned_alloc(64) {
        Ok(ptr) => {
            aligned_free(ptr);
            true
        }
        Err(_) => false,
    }
}

/// Get FlexRAN version info (placeholder)
pub fn version_info() -> &'static str {
    "FlexRAN SDK (Mock Implementation)"
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_flexran_availability() {
        // This will return true in our mock implementation
        assert!(is_available());
    }
    
    #[test]
    fn test_fft_complex_conversion() {
        let c = Complex32::new(1.0, 2.0);
        let ffi: FftComplex = c.into();
        assert_eq!(ffi.re, 1.0);
        assert_eq!(ffi.im, 2.0);
        
        let c2: Complex32 = ffi.into();
        assert_eq!(c2.re, 1.0);
        assert_eq!(c2.im, 2.0);
    }
}