//! Test OFDM backend selection with FlexRAN
//! 
//! This example demonstrates automatic backend selection between
//! FlexRAN hardware acceleration and software FFTW

use layers::phy::{ResourceGrid, CyclicPrefix};
use layers::phy::ofdm_backend::UnifiedOfdmModulator;
use common::types::{SubcarrierSpacing, Bandwidth};
use num_complex::Complex32;
use std::time::Instant;

fn main() {
    println!("OFDM Backend Selection Test");
    println!("============================\n");
    
    // Test configuration
    let fft_size = 2048;
    let cp_type = CyclicPrefix::Normal;
    let scs = SubcarrierSpacing::Scs15;
    let bw = Bandwidth::Bw10;
    
    println!("Configuration:");
    println!("  FFT Size: {}", fft_size);
    println!("  CP Type: {:?}", cp_type);
    println!("  SCS: {:?}", scs);
    println!("  Bandwidth: {:?}", bw);
    
    // Check environment
    println!("\nEnvironment:");
    match std::env::var("FLEXRAN_SDK_DIR") {
        Ok(dir) => println!("  FLEXRAN_SDK_DIR: {}", dir),
        Err(_) => println!("  FLEXRAN_SDK_DIR: not set"),
    }
    
    #[cfg(feature = "flexran")]
    println!("  FlexRAN feature: enabled");
    #[cfg(not(feature = "flexran"))]
    println!("  FlexRAN feature: disabled");
    
    // Create OFDM modulator
    println!("\nCreating OFDM modulator...");
    let modulator = match UnifiedOfdmModulator::new(fft_size, cp_type, scs) {
        Ok(mod_) => mod_,
        Err(e) => {
            eprintln!("Failed to create OFDM modulator: {}", e);
            return;
        }
    };
    
    println!("✓ Backend selected: {}", modulator.backend_type());
    println!("  Hardware accelerated: {}", modulator.is_accelerated());
    
    // Create resource grid
    println!("\nCreating resource grid...");
    let mut resource_grid = match ResourceGrid::new(fft_size, 14, bw, scs) {
        Ok(grid) => grid,
        Err(e) => {
            eprintln!("Failed to create resource grid: {}", e);
            return;
        }
    };
    
    // Fill resource grid with test data
    println!("Filling resource grid with test data...");
    let num_rb = match bw {
        Bandwidth::Bw5 => 25,
        Bandwidth::Bw10 => 52,
        Bandwidth::Bw15 => 79,
        Bandwidth::Bw20 => 106,
        Bandwidth::Bw25 => 133,
        _ => 52,
    };
    
    let num_sc = num_rb * 12;
    
    // Fill with QPSK-like symbols
    for symbol in 0..14 {
        for sc in 0..num_sc {
            let value = if (sc + symbol) % 2 == 0 {
                Complex32::new(0.707, 0.707)
            } else {
                Complex32::new(-0.707, 0.707)
            };
            // Subcarrier indices are 0 to num_sc-1 for the grid
            resource_grid.map_re(sc as u16, symbol as u8, value).unwrap();
        }
    }
    
    println!("✓ Resource grid filled");
    
    // Perform OFDM modulation benchmark
    println!("\nPerforming OFDM modulation benchmark...");
    let num_iterations = 100;
    let mut total_duration = std::time::Duration::new(0, 0);
    
    for i in 0..num_iterations {
        let start = Instant::now();
        let _samples = modulator.modulate(&resource_grid, (i % 14) as u8);
        total_duration += start.elapsed();
    }
    
    let avg_duration = total_duration / num_iterations;
    println!("✓ Average modulation time: {:?}", avg_duration);
    println!("  Throughput: {:.2} symbols/sec", 1_000_000.0 / avg_duration.as_micros() as f64);
    
    // Test slot modulation
    println!("\nTesting slot modulation...");
    let start = Instant::now();
    let slot_samples = modulator.modulate_slot(&resource_grid);
    let slot_duration = start.elapsed();
    
    println!("✓ Slot modulation completed in {:?}", slot_duration);
    println!("  Total samples: {}", slot_samples.len());
    println!("  Samples per symbol: {}", slot_samples.len() / 14);
    
    // Verify sample power
    let power: f32 = slot_samples.iter()
        .map(|s| s.norm_sqr())
        .sum::<f32>() / slot_samples.len() as f32;
    let power_db = 10.0 * power.log10();
    println!("  Average power: {:.2} dB", power_db);
    
    println!("\n============================");
    println!("Test completed successfully!");
}