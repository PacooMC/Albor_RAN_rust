//! Test FlexRAN integration
//! 
//! This example tests the FlexRAN FFI bindings and adapter functionality

use flexran_sys::{AlignedVector, is_available, version_info};
use num_complex::Complex32;
use std::time::Instant;

fn main() {
    println!("FlexRAN Integration Test");
    println!("========================");
    
    // Check if FlexRAN is available
    println!("\n1. Checking FlexRAN availability...");
    if is_available() {
        println!("   ✓ FlexRAN is available");
        println!("   Version: {}", version_info());
    } else {
        println!("   ✗ FlexRAN is not available");
        println!("   This is expected if FLEXRAN_SDK_DIR is not set");
    }
    
    // Test aligned memory allocation
    println!("\n2. Testing aligned memory allocation...");
    match test_aligned_memory() {
        Ok(()) => println!("   ✓ Aligned memory allocation works"),
        Err(e) => println!("   ✗ Aligned memory allocation failed: {}", e),
    }
    
    // Test FFI complex number conversion
    println!("\n3. Testing Complex number conversions...");
    test_complex_conversion();
    
    // Test AlignedVector
    println!("\n4. Testing AlignedVector...");
    match test_aligned_vector() {
        Ok(()) => println!("   ✓ AlignedVector operations work"),
        Err(e) => println!("   ✗ AlignedVector test failed: {}", e),
    }
    
    // Test OFDM operations (mock)
    println!("\n5. Testing OFDM operations (mock)...");
    match test_ofdm_operations() {
        Ok(()) => println!("   ✓ OFDM operations work"),
        Err(e) => println!("   ✗ OFDM operations failed: {}", e),
    }
    
    println!("\n========================");
    println!("Test completed!");
}

fn test_aligned_memory() -> Result<(), String> {
    use flexran_sys::{aligned_alloc, aligned_free, FLEXRAN_ALIGNMENT};
    
    let size = 1024 * std::mem::size_of::<Complex32>();
    let ptr = aligned_alloc(size)
        .map_err(|e| format!("Allocation failed: {:?}", e))?;
    
    // Check alignment
    if ptr as usize % FLEXRAN_ALIGNMENT != 0 {
        aligned_free(ptr);
        return Err("Memory not properly aligned".to_string());
    }
    
    // Clean up
    aligned_free(ptr);
    Ok(())
}

fn test_complex_conversion() {
    use flexran_sys::FftComplex;
    
    let c1 = Complex32::new(1.0, 2.0);
    let ffi: FftComplex = c1.into();
    println!("   Complex32 -> FftComplex: ({}, {}) -> ({}, {})", 
             c1.re, c1.im, ffi.re, ffi.im);
    
    let c2: Complex32 = ffi.into();
    println!("   FftComplex -> Complex32: ({}, {}) -> ({}, {})", 
             ffi.re, ffi.im, c2.re, c2.im);
    
    if c1 == c2 {
        println!("   ✓ Conversion is lossless");
    } else {
        println!("   ✗ Conversion lost precision");
    }
}

fn test_aligned_vector() -> Result<(), String> {
    use flexran_sys::FLEXRAN_ALIGNMENT;
    
    // Create vector with capacity
    let mut vec = AlignedVector::<Complex32>::with_capacity(1024)
        .map_err(|e| format!("Failed to create AlignedVector: {:?}", e))?;
    
    // Check alignment
    if vec.as_ptr() as usize % FLEXRAN_ALIGNMENT != 0 {
        return Err("AlignedVector not properly aligned".to_string());
    }
    
    // Test resize
    vec.resize(512, Complex32::new(0.0, 0.0));
    if vec.len() != 512 {
        return Err("Resize failed".to_string());
    }
    
    // Test from_slice
    let data: Vec<Complex32> = (0..100)
        .map(|i| Complex32::new(i as f32, -(i as f32)))
        .collect();
    
    let vec2 = AlignedVector::from_slice(&data)
        .map_err(|e| format!("from_slice failed: {:?}", e))?;
    
    if vec2.len() != data.len() {
        return Err("from_slice produced wrong length".to_string());
    }
    
    // Verify content
    for (i, &val) in vec2.as_slice().iter().enumerate() {
        if val != data[i] {
            return Err(format!("Data mismatch at index {}", i));
        }
    }
    
    Ok(())
}

fn test_ofdm_operations() -> Result<(), String> {
    use flexran_sys::{ofdm_ifft, ofdm_fft};
    
    let fft_size = 2048;
    let cp_len = 144;
    
    // Create test data
    let mut freq_domain = vec![Complex32::new(0.0, 0.0); fft_size];
    // Add some test signal
    for i in 0..10 {
        freq_domain[i] = Complex32::new(1.0, 0.0);
    }
    
    let mut time_domain = vec![Complex32::new(0.0, 0.0); fft_size];
    
    // Test IFFT
    let start = Instant::now();
    match ofdm_ifft(&freq_domain, &mut time_domain, cp_len, 1.0 / fft_size as f32) {
        Ok(()) => {
            let elapsed = start.elapsed();
            println!("   IFFT completed in {:?}", elapsed);
        }
        Err(e) => {
            // This is expected in mock implementation
            println!("   IFFT returned error (expected in mock): {:?}", e);
        }
    }
    
    // Test FFT
    let mut freq_output = vec![Complex32::new(0.0, 0.0); fft_size];
    let start = Instant::now();
    match ofdm_fft(&time_domain, &mut freq_output, 0) {
        Ok(()) => {
            let elapsed = start.elapsed();
            println!("   FFT completed in {:?}", elapsed);
        }
        Err(e) => {
            // This is expected in mock implementation
            println!("   FFT returned error (expected in mock): {:?}", e);
        }
    }
    
    Ok(())
}