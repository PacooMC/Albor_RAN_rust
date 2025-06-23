/// Polar encoding implementation for 5G NR
/// Based on 3GPP TS 38.212 Section 5.3.1

use tracing::debug;

/// Maximum Polar code length (log2)
pub const NMAX_LOG: usize = 10;
/// Maximum Polar code length
pub const NMAX: usize = 1 << NMAX_LOG;

/// Polar code structure
pub struct PolarCode {
    /// Code length (N)
    n: usize,
    /// Information bits length (K)
    k: usize,
    /// Target output length (E)
    e: usize,
    /// Code length in log2
    n_log: usize,
    /// Frozen bit positions (0 = frozen, 1 = information)
    frozen_bits: Vec<bool>,
    /// Reliability sequence for bit allocation
    reliability_sequence: Vec<usize>,
    /// Block interleaver pattern
    block_interleaver: Vec<usize>,
}

impl PolarCode {
    /// Create a new Polar code
    pub fn new(k: usize, e: usize, n_max_log: usize) -> Self {
        // Calculate code length N
        let n_log = Self::calculate_n_log(k, e, n_max_log);
        let n = 1 << n_log;
        
        // Generate reliability sequence
        let reliability_sequence = Self::generate_reliability_sequence(n);
        
        // Allocate frozen/information bits
        let frozen_bits = Self::allocate_bits(n, k, &reliability_sequence);
        
        // Generate block interleaver pattern
        let block_interleaver = Self::generate_block_interleaver(n);
        
        Self {
            n,
            k,
            e,
            n_log,
            frozen_bits,
            reliability_sequence,
            block_interleaver,
        }
    }
    
    /// Calculate N (code length) based on K and E
    fn calculate_n_log(k: usize, e: usize, n_max_log: usize) -> usize {
        // Find minimum N such that N >= K and N >= E/2
        let min_n = k.max(e / 2);
        
        for n_log in 5..=n_max_log {
            let n = 1 << n_log;
            if n >= min_n {
                return n_log;
            }
        }
        
        n_max_log
    }
    
    /// Generate reliability sequence using 5G method
    fn generate_reliability_sequence(n: usize) -> Vec<usize> {
        let mut w = vec![0f64; n];
        let n_log = (n as f64).log2() as usize;
        
        // Initialize with bit reversal weight
        for j in 0..n {
            let j_rev = Self::bit_reversal(j, n_log);
            w[j] = j_rev as f64;
        }
        
        // Apply polarization weight
        for s in 1..=n_log {
            let increment = 1 << (n_log - s);
            for j in 0..increment {
                for t in 0..(1 << (s - 1)) {
                    let idx1 = j + t * 2 * increment;
                    let idx2 = idx1 + increment;
                    
                    let w1 = w[idx1];
                    let w2 = w[idx2];
                    
                    w[idx1] = w1 + w2;
                    w[idx2] = w2;
                }
            }
        }
        
        // Sort indices by reliability (lowest to highest)
        let mut indices: Vec<usize> = (0..n).collect();
        indices.sort_by(|&a, &b| w[a].partial_cmp(&w[b]).unwrap());
        
        indices
    }
    
    /// Bit reversal
    fn bit_reversal(val: usize, n_bits: usize) -> usize {
        let mut result = 0;
        let mut v = val;
        
        for i in 0..n_bits {
            result = (result << 1) | (v & 1);
            v >>= 1;
        }
        
        result
    }
    
    /// Allocate frozen and information bits
    fn allocate_bits(n: usize, k: usize, reliability_sequence: &[usize]) -> Vec<bool> {
        let mut frozen_bits = vec![false; n]; // All frozen initially
        
        // Set K most reliable bits as information bits
        for i in (n - k)..n {
            frozen_bits[reliability_sequence[i]] = true;
        }
        
        frozen_bits
    }
    
    pub fn get_n(&self) -> usize {
        self.n
    }
    
    pub fn get_k(&self) -> usize {
        self.k
    }
    
    pub fn get_frozen_bits(&self) -> &[bool] {
        &self.frozen_bits
    }
    
    pub fn get_e(&self) -> usize {
        self.e
    }
    
    pub fn get_block_interleaver(&self) -> &[usize] {
        &self.block_interleaver
    }
    
    /// Generate block interleaver pattern for rate matching
    fn generate_block_interleaver(n: usize) -> Vec<usize> {
        let mut pattern = vec![0; n];
        
        // Implement the standard 5G NR block interleaver
        // Based on sub-block interleaving with 32 columns
        let p_il = if n >= 32 {
            let j_max = n / 32;
            let mut idx = 0;
            
            for k in 0..n {
                let i = k / j_max;
                let j = k % j_max;
                let k_prime = i + 32 * j;
                
                if k_prime < n {
                    pattern[idx] = k_prime;
                    idx += 1;
                }
            }
            
            pattern
        } else {
            // No interleaving for small N
            (0..n).collect()
        };
        
        p_il
    }
}

/// Polar interleaver for 5G NR
pub struct PolarInterleaver;

impl PolarInterleaver {
    /// Interleave bits according to 5G NR specification
    pub fn interleave(output: &mut [u8], input: &[u8]) {
        let k = input.len();
        
        // Implement sub-block interleaver as per TS 38.212 Section 5.3.1.1
        if k >= 32 {
            // Apply sub-block interleaving
            let rows = 32;
            let cols = (k + rows - 1) / rows;
            let mut matrix = vec![vec![2u8; cols]; rows]; // 2 = NULL
            
            // Write input row by row
            let mut idx = 0;
            for r in 0..rows {
                for c in 0..cols {
                    if idx < k {
                        matrix[r][c] = input[idx];
                        idx += 1;
                    }
                }
            }
            
            // Read output column by column
            idx = 0;
            for c in 0..cols {
                for r in 0..rows {
                    if matrix[r][c] != 2 {
                        output[idx] = matrix[r][c];
                        idx += 1;
                    }
                }
            }
        } else {
            // No interleaving for K < 32
            output.copy_from_slice(input);
        }
    }
}

/// Polar allocator
pub struct PolarAllocator;

impl PolarAllocator {
    /// Allocate information bits to Polar code
    pub fn allocate(output: &mut [u8], input: &[u8], code: &PolarCode) {
        // Clear output
        output.fill(0);
        
        // Place information bits in non-frozen positions
        let mut info_idx = 0;
        for (i, &is_info) in code.frozen_bits.iter().enumerate() {
            if is_info && info_idx < input.len() {
                output[i] = input[info_idx];
                info_idx += 1;
            }
        }
    }
}

/// Polar encoder
pub struct PolarEncoder;

impl PolarEncoder {
    /// Encode using Polar transform
    pub fn encode(output: &mut [u8], input: &[u8], n_log: usize) {
        // Copy input to output
        output[..input.len()].copy_from_slice(input);
        
        // Apply Polar transform (successive cancellation)
        for s in 1..=n_log {
            let half_stage = 1 << (s - 1);
            let full_stage = 1 << s;
            
            for j in (0..(1 << n_log)).step_by(full_stage) {
                for i in 0..half_stage {
                    let u1_idx = j + i;
                    let u2_idx = j + i + half_stage;
                    
                    output[u1_idx] ^= output[u2_idx];
                }
            }
        }
    }
}

/// Polar rate matcher
pub struct PolarRateMatcher;

impl PolarRateMatcher {
    /// Rate match Polar encoded bits
    pub fn rate_match(output: &mut [u8], input: &[u8], code: &PolarCode) {
        let n = code.get_n();
        let e = code.get_e();
        let k = code.get_k();
        
        // Apply block interleaving first
        let mut interleaved = vec![0u8; n];
        let interleaver = code.get_block_interleaver();
        for (i, &idx) in interleaver.iter().enumerate() {
            interleaved[i] = input[idx];
        }
        
        // Bit selection
        let selected = if e >= n {
            // Repetition
            let mut repeated = vec![0u8; e];
            for i in 0..e {
                repeated[i] = interleaved[i % n];
            }
            repeated
        } else {
            // Puncturing or shortening
            if 16 * k <= 7 * e {
                // Puncturing (from the beginning)
                interleaved[(n - e)..].to_vec()
            } else {
                // Shortening (from the end)
                interleaved[..e].to_vec()
            }
        };
        
        // Channel interleaving (only for DCI)
        // For PDCCH, we apply channel interleaving
        Self::channel_interleave(output, &selected, e);
        
        debug!("Rate matched {} bits to {} bits", n, e);
    }
    
    /// Channel interleaver for rate matching
    fn channel_interleave(output: &mut [u8], input: &[u8], e: usize) {
        // Calculate T - smallest integer such that T(T+1)/2 >= E
        let mut t = 1;
        let mut s = 1;
        while s < e {
            t += 1;
            s += t;
        }
        
        // Perform triangular interleaving
        let mut out_idx = 0;
        for r in 0..t {
            let mut in_idx = r;
            for c in 0..(t - r) {
                if in_idx < e {
                    output[out_idx] = input[in_idx];
                    out_idx += 1;
                    in_idx += t - c;
                } else {
                    break;
                }
            }
        }
    }
}

/// Complete Polar encoder for PDCCH
pub struct PdcchPolarEncoder {
    interleaver: PolarInterleaver,
    allocator: PolarAllocator,
    encoder: PolarEncoder,
    rate_matcher: PolarRateMatcher,
}

impl PdcchPolarEncoder {
    pub fn new() -> Self {
        Self {
            interleaver: PolarInterleaver,
            allocator: PolarAllocator,
            encoder: PolarEncoder,
            rate_matcher: PolarRateMatcher,
        }
    }
    
    /// Encode PDCCH payload with Polar code
    pub fn encode(&self, payload_with_crc: &[u8], aggregation_level: u8) -> Vec<u8> {
        // Calculate E (number of encoded bits)
        let e = aggregation_level as usize * 6 * 12 * 2; // CCEs * REGs/CCE * REs/REG * bits/RE
        let k = payload_with_crc.len();
        
        // Create Polar code
        let code = PolarCode::new(k, e, NMAX_LOG - 1);
        let n = code.get_n();
        
        debug!("Polar encoding: K={}, E={}, N={}", k, e, n);
        
        // 1. Interleave
        let mut interleaved = vec![0u8; k];
        PolarInterleaver::interleave(&mut interleaved, payload_with_crc);
        
        // 2. Allocate bits
        let mut allocated = vec![0u8; n];
        PolarAllocator::allocate(&mut allocated, &interleaved, &code);
        
        // 3. Encode
        let mut encoded = vec![0u8; n];
        PolarEncoder::encode(&mut encoded, &allocated, code.n_log);
        
        // 4. Rate match
        let mut rate_matched = vec![0u8; e];
        PolarRateMatcher::rate_match(&mut rate_matched, &encoded, &code);
        
        rate_matched
    }
}