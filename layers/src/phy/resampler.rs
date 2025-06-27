//! Sample Rate Resampler for PHY-ZMQ Interface
//! 
//! Implements fractional resampling between PHY sample rate (15.36 MHz)
//! and ZMQ interface sample rate (11.52 MHz) with proper anti-aliasing.
//! 
//! Ratio: 15.36/11.52 = 4/3 (downsample by 4:3)

use num_complex::Complex32;
use std::f32::consts::PI;
use tracing::{debug, info};

/// Resampler configuration
#[derive(Debug, Clone)]
pub struct ResamplerConfig {
    /// Input sample rate (Hz)
    pub input_rate: f64,
    /// Output sample rate (Hz)  
    pub output_rate: f64,
    /// Filter order (number of taps)
    pub filter_order: usize,
    /// Filter cutoff frequency as fraction of Nyquist
    pub cutoff_factor: f32,
}

impl Default for ResamplerConfig {
    fn default() -> Self {
        Self {
            input_rate: 15.36e6,   // PHY rate
            output_rate: 11.52e6,  // ZMQ rate
            filter_order: 64,      // Good balance of quality and performance
            cutoff_factor: 0.45,   // Conservative cutoff to prevent aliasing
        }
    }
}

/// Polyphase FIR resampler for efficient fractional rate conversion
pub struct Resampler {
    config: ResamplerConfig,
    /// Interpolation factor (L)
    interp_factor: usize,
    /// Decimation factor (M)
    decim_factor: usize,
    /// Polyphase filter coefficients [phase][tap]
    polyphase_filters: Vec<Vec<f32>>,
    /// Delay line for filtering
    delay_line: Vec<Complex32>,
    /// Current phase index
    phase_index: usize,
    /// Accumulated input samples
    sample_accumulator: usize,
}

impl Resampler {
    /// Create a new resampler
    pub fn new(config: ResamplerConfig) -> Self {
        // Calculate rational approximation of rate ratio
        let ratio = config.output_rate / config.input_rate;
        let (interp, decim) = rational_approximation(ratio, 1000);
        
        info!("Resampler: {} MHz -> {} MHz, ratio={:.6} ≈ {}/{}", 
              config.input_rate / 1e6, config.output_rate / 1e6, ratio, interp, decim);
        
        // Design lowpass filter
        let cutoff_hz = config.cutoff_factor as f64 * config.output_rate.min(config.input_rate) / 2.0;
        let filter_taps = design_lowpass_filter(
            config.filter_order * interp,
            cutoff_hz,
            config.input_rate * interp as f64,
        );
        
        // Apply interpolation gain compensation
        let gain = interp as f32;
        let filter_taps: Vec<f32> = filter_taps.iter().map(|&x| x * gain).collect();
        
        // Create polyphase decomposition
        let mut polyphase_filters = vec![vec![0.0; config.filter_order]; interp];
        for (i, &tap) in filter_taps.iter().enumerate() {
            let phase = i % interp;
            let tap_idx = i / interp;
            if tap_idx < config.filter_order {
                polyphase_filters[phase][tap_idx] = tap;
            }
        }
        
        // Initialize delay line
        let delay_line = vec![Complex32::new(0.0, 0.0); config.filter_order];
        
        debug!("Resampler initialized: L={}, M={}, {} taps per phase", 
               interp, decim, config.filter_order);
        
        Self {
            config,
            interp_factor: interp,
            decim_factor: decim,
            polyphase_filters,
            delay_line,
            phase_index: 0,
            sample_accumulator: 0,
        }
    }
    
    /// Process a block of samples
    pub fn process(&mut self, input: &[Complex32]) -> Vec<Complex32> {
        let mut output = Vec::new();
        
        for &sample in input {
            // Shift delay line and insert new sample
            self.delay_line.rotate_right(1);
            self.delay_line[0] = sample;
            
            // Produce output samples while we can
            while self.sample_accumulator < self.decim_factor {
                // Apply polyphase filter for current phase
                let filter = &self.polyphase_filters[self.phase_index];
                let mut out_sample = Complex32::new(0.0, 0.0);
                
                for (i, &coeff) in filter.iter().enumerate() {
                    out_sample += self.delay_line[i] * coeff;
                }
                
                output.push(out_sample);
                
                // Update phase
                self.sample_accumulator += self.interp_factor;
                self.phase_index = (self.phase_index + self.interp_factor) % self.interp_factor;
            }
            
            // Consume input sample
            self.sample_accumulator -= self.decim_factor;
        }
        
        output
    }
    
    /// Get the expected output size for a given input size
    pub fn get_output_size(&self, input_size: usize) -> usize {
        (input_size * self.interp_factor + self.sample_accumulator) / self.decim_factor
    }
    
    /// Reset the resampler state
    pub fn reset(&mut self) {
        self.delay_line.fill(Complex32::new(0.0, 0.0));
        self.phase_index = 0;
        self.sample_accumulator = 0;
    }
}

/// Find rational approximation of a decimal number
fn rational_approximation(value: f64, max_denominator: usize) -> (usize, usize) {
    // Special case for our known ratio
    if (value - 0.75).abs() < 1e-6 {
        return (3, 4); // 11.52/15.36 = 3/4
    }
    
    // General case using continued fractions
    let mut a = value.floor() as i64;
    let mut h1 = 1i64;
    let mut k1 = 0i64;
    let mut h = a;
    let mut k = 1i64;
    
    let mut remainder = value - a as f64;
    
    while k <= max_denominator as i64 && remainder.abs() > 1e-10 {
        let x = 1.0 / remainder;
        a = x.floor() as i64;
        remainder = x - a as f64;
        
        let h_temp = h;
        let k_temp = k;
        h = a * h + h1;
        k = a * k + k1;
        h1 = h_temp;
        k1 = k_temp;
    }
    
    (h.abs() as usize, k.abs() as usize)
}

/// Design lowpass FIR filter using windowed sinc method
fn design_lowpass_filter(num_taps: usize, cutoff_hz: f64, sample_rate: f64) -> Vec<f32> {
    let mut taps = vec![0.0f32; num_taps];
    let center = (num_taps - 1) as f32 / 2.0;
    let omega_c = 2.0 * PI * (cutoff_hz / sample_rate) as f32;
    
    for i in 0..num_taps {
        let n = i as f32 - center;
        
        // Sinc function
        let sinc = if n.abs() < 1e-10 {
            omega_c / PI
        } else {
            (omega_c * n).sin() / (PI * n)
        };
        
        // Hamming window
        let window = 0.54 - 0.46 * (2.0 * PI * i as f32 / (num_taps - 1) as f32).cos();
        
        taps[i] = sinc * window;
    }
    
    // Normalize filter
    let sum: f32 = taps.iter().sum();
    for tap in &mut taps {
        *tap /= sum;
    }
    
    taps
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_rational_approximation() {
        // Test our specific ratio
        let (num, den) = rational_approximation(0.75, 100);
        assert_eq!((num, den), (3, 4));
        
        // Test other common ratios
        let (num, den) = rational_approximation(0.5, 100);
        assert_eq!((num, den), (1, 2));
        
        let (num, den) = rational_approximation(0.333333, 100);
        assert_eq!((num, den), (1, 3));
    }
    
    #[test]
    fn test_resampler_output_size() {
        let config = ResamplerConfig::default();
        let mut resampler = Resampler::new(config);
        
        // For 4:3 downsampling, 4 input samples should produce 3 output samples
        assert_eq!(resampler.get_output_size(4), 3);
        assert_eq!(resampler.get_output_size(8), 6);
        assert_eq!(resampler.get_output_size(1024), 768);
    }
    
    #[test]
    fn test_resampler_dc_passthrough() {
        let config = ResamplerConfig::default();
        let mut resampler = Resampler::new(config);
        
        // DC signal should pass through with minimal attenuation
        let dc_signal = vec![Complex32::new(1.0, 0.0); 1024];
        let output = resampler.process(&dc_signal);
        
        // Check output length
        assert_eq!(output.len(), 768); // 1024 * 3/4
        
        // Check DC preservation (allow for some filter settling time)
        let settled_output = &output[100..];
        let avg_real: f32 = settled_output.iter().map(|s| s.re).sum::<f32>() / settled_output.len() as f32;
        assert!((avg_real - 1.0).abs() < 0.1, "DC not preserved: {}", avg_real);
    }
}