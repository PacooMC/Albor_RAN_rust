use num_complex::Complex32;
use fftw::array::AlignedVec;
use fftw::plan::{C2CPlan32, C2CPlan};
use fftw::types::{Flag, Sign};

fn main() {
    println!("Testing IFFT implementation...");
    
    let fft_size = 1024;
    
    // Create IFFT plan
    println!("Creating IFFT plan for size {}...", fft_size);
    let mut ifft_input = AlignedVec::new(fft_size);
    let mut ifft_output = AlignedVec::new(fft_size);
    
    let ifft_plan = C2CPlan32::aligned(
        &[fft_size],
        Sign::Backward,
        Flag::ESTIMATE,
    ).expect("Failed to create IFFT plan");
    
    println!("IFFT plan created successfully");
    
    // Test 1: Single tone at DC
    println!("\nTest 1: Single tone at DC");
    for i in 0..fft_size {
        ifft_input[i] = num_complex::Complex::new(0.0, 0.0);
    }
    ifft_input[fft_size / 2] = num_complex::Complex::new(1.0, 0.0);
    
    println!("Input: single tone at bin {} with amplitude 1.0", fft_size / 2);
    
    // Execute IFFT
    ifft_plan.c2c(&mut ifft_input, &mut ifft_output).unwrap();
    
    // Check output
    let power: f32 = ifft_output.iter().map(|s| s.re * s.re + s.im * s.im).sum::<f32>() / ifft_output.len() as f32;
    let power_db = 10.0 * power.log10();
    println!("Output power: {:.2} dB", power_db);
    println!("First 10 samples:");
    for i in 0..10 {
        println!("  [{}] = {:.6} + {:.6}j", i, ifft_output[i].re, ifft_output[i].im);
    }
    
    // Test 2: Multiple tones
    println!("\nTest 2: Multiple tones");
    for i in 0..fft_size {
        ifft_input[i] = num_complex::Complex::new(0.0, 0.0);
    }
    // Add tones at different frequencies
    ifft_input[fft_size / 2 - 10] = num_complex::Complex::new(0.5, 0.0);
    ifft_input[fft_size / 2] = num_complex::Complex::new(1.0, 0.0);
    ifft_input[fft_size / 2 + 10] = num_complex::Complex::new(0.5, 0.0);
    
    println!("Input: 3 tones at bins {}, {}, {}", fft_size/2 - 10, fft_size/2, fft_size/2 + 10);
    
    // Execute IFFT
    ifft_plan.c2c(&mut ifft_input, &mut ifft_output).unwrap();
    
    // Check output
    let power2: f32 = ifft_output.iter().map(|s| s.re * s.re + s.im * s.im).sum::<f32>() / ifft_output.len() as f32;
    let power_db2 = 10.0 * power2.log10();
    println!("Output power: {:.2} dB", power_db2);
    println!("First 10 samples:");
    for i in 0..10 {
        println!("  [{}] = {:.6} + {:.6}j", i, ifft_output[i].re, ifft_output[i].im);
    }
    
    // Test 3: PSS-like signal (127 subcarriers)
    println!("\nTest 3: PSS-like signal");
    for i in 0..fft_size {
        ifft_input[i] = num_complex::Complex::new(0.0, 0.0);
    }
    
    // Map 127 subcarriers around DC
    let pss_start = fft_size / 2 - 63;
    for i in 0..127 {
        ifft_input[pss_start + i] = num_complex::Complex::new(1.0 / (127.0_f32).sqrt(), 0.0);
    }
    
    println!("Input: 127 subcarriers around DC with normalized amplitude");
    
    // Execute IFFT
    ifft_plan.c2c(&mut ifft_input, &mut ifft_output).unwrap();
    
    // Check output
    let power3: f32 = ifft_output.iter().map(|s| s.re * s.re + s.im * s.im).sum::<f32>() / ifft_output.len() as f32;
    let power_db3 = 10.0 * power3.log10();
    println!("Output power: {:.2} dB", power_db3);
    println!("First 10 samples:");
    for i in 0..10 {
        println!("  [{}] = {:.6} + {:.6}j", i, ifft_output[i].re, ifft_output[i].im);
    }
    
    println!("\nAll tests completed!");
}