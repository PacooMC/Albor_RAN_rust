//! Physical Broadcast Channel (PBCH) Processing
//! 
//! Implements PBCH encoding/decoding according to 3GPP TS 38.212

use crate::LayerError;
use crate::phy::polar::{PolarCode, PolarInterleaver, PolarAllocator, PolarEncoder, PolarRateMatcher, NMAX_LOG};
use common::types::{Pci, CellId, SubcarrierSpacing};
use num_complex::Complex32;
use serde::{Serialize, Deserialize};
use tracing::debug;

/// MIB (Master Information Block) structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mib {
    /// System frame number (6 bits)
    pub sfn: u16,
    /// Subcarrier spacing for common RBs (1 bit)
    pub subcarrier_spacing_common: SubcarrierSpacing,
    /// SSB subcarrier offset (4 bits)
    pub ssb_subcarrier_offset: u8,
    /// DMRS position for PDSCH (1 bit)
    pub dmrs_type_a_position: DmrsPosition,
    /// PDCCH config SIB1 (8 bits)
    pub pdcch_config_sib1: u8,
    /// Cell barred (1 bit)
    pub cell_barred: bool,
    /// Intra frequency reselection (1 bit)
    pub intra_freq_reselection: bool,
    /// Spare bit
    pub spare: u8,
}

/// DMRS position for PDSCH
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DmrsPosition {
    Pos2,
    Pos3,
}

impl Mib {
    /// Create a new MIB
    pub fn new() -> Self {
        Self {
            sfn: 0,
            subcarrier_spacing_common: SubcarrierSpacing::Scs15,
            ssb_subcarrier_offset: 0,
            dmrs_type_a_position: DmrsPosition::Pos2,
            pdcch_config_sib1: 0,
            cell_barred: false,
            intra_freq_reselection: true,
            spare: 0,
        }
    }
    
    /// Encode MIB to bits (24 bits total)
    pub fn encode(&self) -> Vec<u8> {
        let mut bits = Vec::with_capacity(24);
        
        // CHOICE in BCCH-BCH-MessageType (1 bit) - always 0 for MIB
        bits.push(0);
        
        // System frame number (6 bits)
        for i in (0..6).rev() {
            bits.push(((self.sfn >> i) & 1) as u8);
        }
        
        // Subcarrier spacing common (1 bit)
        let scs_bit = match self.subcarrier_spacing_common {
            SubcarrierSpacing::Scs15 => 0,
            SubcarrierSpacing::Scs30 => 1,
            _ => 0,
        };
        bits.push(scs_bit);
        
        // SSB subcarrier offset (4 bits)
        for i in (0..4).rev() {
            bits.push(((self.ssb_subcarrier_offset >> i) & 1) as u8);
        }
        
        // DMRS position (1 bit)
        bits.push(match self.dmrs_type_a_position {
            DmrsPosition::Pos2 => 0,
            DmrsPosition::Pos3 => 1,
        });
        
        // PDCCH config SIB1 (8 bits)
        for i in (0..8).rev() {
            bits.push(((self.pdcch_config_sib1 >> i) & 1) as u8);
        }
        
        // Cell barred (1 bit)
        bits.push(if self.cell_barred { 1 } else { 0 });
        
        // Intra frequency reselection (1 bit)
        bits.push(if self.intra_freq_reselection { 1 } else { 0 });
        
        // Spare (1 bit)
        bits.push(self.spare & 1);
        
        bits
    }
    
    /// Decode MIB from bits
    pub fn decode(bits: &[u8]) -> Result<Self, LayerError> {
        if bits.len() != 24 {
            return Err(LayerError::InvalidConfiguration(
                format!("MIB must be 24 bits, got {}", bits.len())
            ));
        }
        
        // Skip CHOICE bit at index 0
        let mut mib = Self::new();
        
        // System frame number (6 bits)
        mib.sfn = 0;
        for i in 1..7 {
            mib.sfn = (mib.sfn << 1) | (bits[i] as u16);
        }
        
        // Subcarrier spacing common (1 bit)
        mib.subcarrier_spacing_common = if bits[7] == 0 {
            SubcarrierSpacing::Scs15
        } else {
            SubcarrierSpacing::Scs30
        };
        
        // SSB subcarrier offset (4 bits)
        mib.ssb_subcarrier_offset = 0;
        for i in 8..12 {
            mib.ssb_subcarrier_offset = (mib.ssb_subcarrier_offset << 1) | bits[i];
        }
        
        // DMRS position (1 bit)
        mib.dmrs_type_a_position = if bits[12] == 0 {
            DmrsPosition::Pos2
        } else {
            DmrsPosition::Pos3
        };
        
        // PDCCH config SIB1 (8 bits)
        mib.pdcch_config_sib1 = 0;
        for i in 13..21 {
            mib.pdcch_config_sib1 = (mib.pdcch_config_sib1 << 1) | bits[i];
        }
        
        // Cell barred (1 bit)
        mib.cell_barred = bits[21] == 1;
        
        // Intra frequency reselection (1 bit)
        mib.intra_freq_reselection = bits[22] == 1;
        
        // Spare (1 bit)
        mib.spare = bits[23];
        
        Ok(mib)
    }
}

/// PBCH processor
#[derive(Clone)]
pub struct PbchProcessor {
    /// Physical cell ID
    pci: Pci,
    /// Cell ID
    cell_id: CellId,
    /// Scrambling sequence cache
    scrambling_cache: Vec<Vec<u8>>,
}

impl PbchProcessor {
    /// PBCH payload size in bits (A-bar according to 3GPP TS 38.212)
    const PBCH_A_BAR_SIZE: usize = 32; // MIB (24) + Additional PBCH payload (8)
    /// PBCH payload size after CRC attachment
    const PBCH_PAYLOAD_SIZE: usize = 56; // A-bar (32) + CRC (24)
    /// PBCH encoded size after polar coding
    const PBCH_ENCODED_SIZE: usize = 864;
    
    /// Create a new PBCH processor
    pub fn new(pci: Pci, cell_id: CellId) -> Result<Self, LayerError> {
        Ok(Self {
            pci,
            cell_id,
            scrambling_cache: Vec::new(),
        })
    }
    
    /// Generate MIB for given frame
    pub fn generate_mib(&self, frame_number: u32) -> Mib {
        let mut mib = Mib::new();
        
        // Set system frame number (6 MSBs of the 10-bit SFN)
        // MIB contains bits 9-4 of SFN (6 bits)
        mib.sfn = ((frame_number >> 4) & 0x3F) as u16;
        
        // Configure based on cell parameters
        mib.subcarrier_spacing_common = SubcarrierSpacing::Scs15;
        mib.ssb_subcarrier_offset = 0;
        mib.dmrs_type_a_position = DmrsPosition::Pos2;
        
        // Configure PDCCH for SIB1
        // For Band 3 FDD with 15 kHz SCS, 10 MHz bandwidth:
        // CORESET#0 table index 6: 48 RBs, 1 symbol, offset 12
        // SearchSpace#0 index 0: standard search space configuration
        // pdcch_config_sib1 = (coreset0_idx << 4) | searchspace0_idx
        mib.pdcch_config_sib1 = (6 << 4) | 0;  // CORESET#0 index 6, SearchSpace#0 index 0 = 0x60
        
        mib.cell_barred = false;
        mib.intra_freq_reselection = true;
        
        mib
    }
    
    /// Encode PBCH payload
    pub fn encode_pbch(&self, mib: &Mib, frame_number: u32) -> Vec<Complex32> {
        // Generate PBCH payload according to TS 38.212
        let a = self.generate_pbch_payload(mib, frame_number);
        
        // Apply scrambling with selective scrambling
        let a_prime = self.scramble_pbch_payload(&a, frame_number);
        
        // Add CRC-24
        let payload_with_crc = self.add_crc24(&a_prime);
        
        // Channel coding (Polar coding)
        let encoded_bits = self.channel_encode(&payload_with_crc);
        
        // Rate matching
        let rate_matched = self.rate_match(&encoded_bits);
        
        // Additional scrambling of rate-matched bits
        let scrambled = self.scramble(&rate_matched, frame_number);
        
        // Modulation (QPSK)
        self.modulate_qpsk(&scrambled)
    }
    
    /// Decode PBCH payload
    pub fn decode_pbch(&self, symbols: &[Complex32], frame_number: u32) -> Result<(Mib, u8), LayerError> {
        // Demodulation
        let soft_bits = self.demodulate_qpsk(symbols);
        
        // Descrambling - use cell ID for initialization
        let descrambled = self.descramble(&soft_bits);
        
        // Rate de-matching
        let dematched = self.rate_dematch(&descrambled);
        
        // Channel decoding
        let decoded = self.channel_decode(&dematched)?;
        
        // CRC check - returns MIB bits
        let mib_bits = self.check_crc(&decoded)?;
        
        // Extract additional bits from A-bar for SFN verification
        let sfn_bits = if decoded.len() >= Self::PBCH_A_BAR_SIZE {
            // Extract SFN bits 2-5 from additional payload
            let mut sfn = 0u8;
            for i in 24..28 {
                sfn = (sfn << 1) | decoded[i];
            }
            sfn
        } else {
            0
        };
        
        // Decode MIB
        let mib = Mib::decode(&mib_bits)?;
        Ok((mib, sfn_bits))
    }
    
    /// Add CRC-24 to PBCH payload
    fn add_crc24(&self, bits: &[u8]) -> Vec<u8> {
        let mut payload = bits.to_vec();
        
        // Calculate 24-bit CRC for PBCH
        let crc = self.calculate_crc24(bits);
        
        // Append CRC bits
        for i in (0..24).rev() {
            payload.push(((crc >> i) & 1) as u8);
        }
        
        payload
    }
    
    /// Calculate 24-bit CRC for PBCH
    fn calculate_crc24(&self, bits: &[u8]) -> u32 {
        // CRC-24C polynomial for PBCH: x^24 + x^23 + x^21 + x^20 + x^17 + x^15 + x^13 + x^12 + x^8 + x^4 + x^2 + x + 1
        let polynomial = 0x1B2B117; // CRC-24C polynomial
        let mut crc = 0u32;
        
        for &bit in bits {
            crc ^= (bit as u32) << 23;
            if crc & 0x800000 != 0 {
                crc = (crc << 1) ^ polynomial;
            } else {
                crc <<= 1;
            }
            crc &= 0xFFFFFF; // Keep only 24 bits
        }
        
        crc
    }
    
    /// Channel encoding using Polar code for PBCH
    fn channel_encode(&self, bits: &[u8]) -> Vec<u8> {
        debug!("PBCH Polar encoding: input {} bits, target {} bits", bits.len(), Self::PBCH_ENCODED_SIZE);
        
        // PBCH uses Polar encoding with:
        // K = 56 (payload with CRC)
        // E = 864 (encoded bits)
        // N will be calculated based on K and E
        
        let k = bits.len(); // Should be 56
        let e = Self::PBCH_ENCODED_SIZE; // 864
        
        // Create Polar code for PBCH
        // For PBCH, use n_max_log = 9 (N_max = 512)
        let code = PolarCode::new(k, e, 9);
        let n = code.get_n();
        
        debug!("PBCH Polar code: K={}, E={}, N={}", k, e, n);
        
        // 1. No interleaving for PBCH (unlike PDCCH)
        // According to TS 38.212, PBCH doesn't use sub-block interleaving
        
        // 2. Allocate bits - place information bits in reliable positions
        let mut allocated = vec![0u8; n];
        PolarAllocator::allocate(&mut allocated, bits, &code);
        
        // 3. Encode using Polar transform
        let mut encoded = vec![0u8; n];
        PolarEncoder::encode(&mut encoded, &allocated, code.get_n_log());
        
        // 4. Rate match to target size E
        let mut rate_matched = vec![0u8; e];
        self.pbch_rate_match(&mut rate_matched, &encoded, &code);
        
        rate_matched
    }
    
    /// PBCH-specific rate matching
    fn pbch_rate_match(&self, output: &mut [u8], input: &[u8], code: &PolarCode) {
        let n = code.get_n();
        let e = code.get_e();
        
        // PBCH rate matching is different from PDCCH
        // No sub-block interleaving for PBCH
        
        if e >= n {
            // Repetition
            for i in 0..e {
                output[i] = input[i % n];
            }
        } else {
            // For PBCH, always use shortening (take first E bits)
            output[..e].copy_from_slice(&input[..e]);
        }
        
        debug!("PBCH rate matched {} bits to {} bits", n, e);
    }
    
    /// Rate matching
    fn rate_match(&self, bits: &[u8]) -> Vec<u8> {
        // For simplicity, just truncate or pad to desired size
        if bits.len() > Self::PBCH_ENCODED_SIZE {
            bits[..Self::PBCH_ENCODED_SIZE].to_vec()
        } else {
            let mut matched = bits.to_vec();
            while matched.len() < Self::PBCH_ENCODED_SIZE {
                matched.push(0);
            }
            matched
        }
    }
    
    /// Generate PBCH payload with G interleaving pattern (TS 38.212 Table 7.1.1-1)
    fn generate_pbch_payload(&self, mib: &Mib, frame_number: u32) -> Vec<u8> {
        // G interleaving pattern from TS 38.212 Table 7.1.1-1
        const G: [usize; 32] = [
            16, 23, 18, 17, 8, 30, 10, 6, 24, 7, 0, 5, 3, 2, 1, 4,
            9, 11, 12, 13, 14, 15, 19, 20, 21, 22, 25, 26, 27, 28, 29, 31
        ];
        
        // Create A array (32 bits)
        let mut a = vec![0u8; 32];
        
        // Get MIB bits (24 bits)
        let mib_bits = mib.encode();
        
        // Place MIB payload with G interleaving
        // According to TS 38.212 and srsRAN implementation:
        // - MIB bits 1-6 (SFN bits) go to G[0] through G[5]
        // - MIB bit 0 (CHOICE) and bits 7-23 go to G[14] through G[31]
        let mut j_sfn = 0;
        let mut j_other = 14;
        
        for i in 0..24 {
            // SFN bits in MIB (bits 1-6) go to special positions
            if i >= 1 && i < 7 {
                a[G[j_sfn]] = mib_bits[i];
                j_sfn += 1;
            } else {
                // MIB bit 0 (CHOICE) and bits 7-23 go to j_other positions
                // This should fill positions G[14] through G[31] (18 positions total)
                if j_other < 32 {
                    a[G[j_other]] = mib_bits[i];
                    j_other += 1;
                }
            }
        }
        
        // j_sfn should now be 6, pointing to G[6]
        // Add 4 LSBs of SFN (bits 0-3) at positions G[6] to G[9]
        a[G[6]] = ((frame_number >> 3) & 1) as u8;  // 4th LSB of SFN
        a[G[7]] = ((frame_number >> 2) & 1) as u8;  // 3rd LSB of SFN
        a[G[8]] = ((frame_number >> 1) & 1) as u8;  // 2nd LSB of SFN
        a[G[9]] = ((frame_number >> 0) & 1) as u8;  // 1st LSB of SFN
        
        // Half-frame bit at G[10]
        let hrf = ((frame_number / 5) % 2) as u8;  // 0 for even half-frame, 1 for odd
        a[G[10]] = hrf;
        
        // For L_max <= 8: k_SSB MSB at G[11], reserved at G[12] and G[13]
        // For our case with L_max = 4, k_SSB = 0
        a[G[11]] = 0;  // k_SSB MSB (5th bit)
        a[G[12]] = 0;  // Reserved
        a[G[13]] = 0;  // Reserved
        
        debug!("Generated PBCH payload: SFN={}, HRF={}", frame_number, hrf);
        
        a
    }
    
    /// Apply selective scrambling to PBCH payload
    fn scramble_pbch_payload(&self, a: &[u8], frame_number: u32) -> Vec<u8> {
        // G interleaving pattern (same as in generate_pbch_payload)
        const G: [usize; 32] = [
            16, 23, 18, 17, 8, 30, 10, 6, 24, 7, 0, 5, 3, 2, 1, 4,
            9, 11, 12, 13, 14, 15, 19, 20, 21, 22, 25, 26, 27, 28, 29, 31
        ];
        
        let mut a_prime = a.to_vec();
        
        // Get 2nd and 3rd LSB of SFN from specific G positions
        // 2nd LSB is at G[8], 3rd LSB is at G[7]
        let sfn_2nd_lsb = a[G[8]];   // G[8] = 24
        let sfn_3rd_lsb = a[G[7]];   // G[7] = 6
        
        // Calculate v = 2 * SFN_3rd_LSB + SFN_2nd_LSB
        let v = 2 * sfn_3rd_lsb + sfn_2nd_lsb;
        
        // Initialize scrambling sequence with PCI (Physical Cell ID)
        let c_init = self.pci.0 as u32;
        
        // For L_max <= 8, M = 29 (32 - 3)
        let m = 29;
        
        // Generate scrambling sequence starting at position M * v
        let offset = (m * v) as usize;
        let scrambling_seq = self.generate_gold_sequence_with_offset(c_init, 32 + offset);
        
        // Apply selective scrambling
        // Don't scramble: HRF (G[10]), 2nd LSB (G[8]), 3rd LSB (G[7])
        // For L_max <= 8, also don't scramble G[11], G[12], G[13]
        let no_scramble_positions = [G[7], G[8], G[10], G[11], G[12], G[13]];
        
        let mut j = 0;
        for i in 0..32 {
            if no_scramble_positions.contains(&i) {
                // Don't scramble these positions
                a_prime[i] = a[i];
            } else {
                // Apply scrambling
                a_prime[i] = a[i] ^ scrambling_seq[offset + j];
                j += 1;
            }
        }
        
        a_prime
    }
    
    /// Generate Gold sequence with offset
    fn generate_gold_sequence_with_offset(&self, c_init: u32, length: usize) -> Vec<u8> {
        let mut sequence = Vec::with_capacity(length);
        
        // Initialize shift registers
        let mut x1 = 1u32;
        let mut x2 = c_init;
        
        // Advance 1600 iterations
        for _ in 0..1600 {
            let x1_new = ((x1 >> 3) ^ x1) & 1;
            let x2_new = ((x2 >> 3) ^ (x2 >> 2) ^ (x2 >> 1) ^ x2) & 1;
            x1 = (x1 >> 1) | (x1_new << 30);
            x2 = (x2 >> 1) | (x2_new << 30);
        }
        
        // Generate sequence
        for _ in 0..length {
            let x1_new = ((x1 >> 3) ^ x1) & 1;
            let x2_new = ((x2 >> 3) ^ (x2 >> 2) ^ (x2 >> 1) ^ x2) & 1;
            sequence.push((x1_new ^ x2_new) as u8);
            x1 = (x1 >> 1) | (x1_new << 30);
            x2 = (x2 >> 1) | (x2_new << 30);
        }
        
        sequence
    }
    
    /// Scrambling with Gold sequence
    fn scramble(&self, bits: &[u8], frame_number: u32) -> Vec<u8> {
        let mut scrambled = Vec::with_capacity(bits.len());
        
        // Initialize scrambling sequence according to 3GPP TS 38.211
        // c_init = N_id (Physical Cell ID) for PBCH
        let c_init = self.pci.0 as u32;
        let scrambling_seq = self.generate_gold_sequence(c_init, bits.len());
        
        // XOR with scrambling sequence
        for (i, &bit) in bits.iter().enumerate() {
            scrambled.push(bit ^ scrambling_seq[i]);
        }
        
        scrambled
    }
    
    /// Generate Gold sequence for scrambling
    fn generate_gold_sequence(&self, c_init: u32, length: usize) -> Vec<u8> {
        let mut sequence = Vec::with_capacity(length);
        
        // Initialize shift registers
        let mut x1 = 1u32;
        let mut x2 = c_init;
        
        // Advance 1600 iterations
        for _ in 0..1600 {
            let x1_new = ((x1 >> 3) ^ x1) & 1;
            let x2_new = ((x2 >> 3) ^ (x2 >> 2) ^ (x2 >> 1) ^ x2) & 1;
            x1 = (x1 >> 1) | (x1_new << 30);
            x2 = (x2 >> 1) | (x2_new << 30);
        }
        
        // Generate sequence
        for _ in 0..length {
            let x1_new = ((x1 >> 3) ^ x1) & 1;
            let x2_new = ((x2 >> 3) ^ (x2 >> 2) ^ (x2 >> 1) ^ x2) & 1;
            sequence.push((x1_new ^ x2_new) as u8);
            x1 = (x1 >> 1) | (x1_new << 30);
            x2 = (x2 >> 1) | (x2_new << 30);
        }
        
        sequence
    }
    
    /// QPSK modulation
    fn modulate_qpsk(&self, bits: &[u8]) -> Vec<Complex32> {
        let mut symbols = Vec::with_capacity(bits.len() / 2);
        
        for i in (0..bits.len()).step_by(2) {
            let b0 = bits[i] as f32;
            let b1 = bits[i + 1] as f32;
            
            // QPSK constellation mapping
            let i_val = (1.0 - 2.0 * b0) / 2.0_f32.sqrt();
            let q_val = (1.0 - 2.0 * b1) / 2.0_f32.sqrt();
            
            symbols.push(Complex32::new(i_val, q_val));
        }
        
        symbols
    }
    
    /// QPSK demodulation (soft decision)
    fn demodulate_qpsk(&self, symbols: &[Complex32]) -> Vec<f32> {
        let mut soft_bits = Vec::with_capacity(symbols.len() * 2);
        
        for symbol in symbols {
            // Soft decision for I component
            soft_bits.push(-symbol.re * 2.0_f32.sqrt());
            // Soft decision for Q component
            soft_bits.push(-symbol.im * 2.0_f32.sqrt());
        }
        
        soft_bits
    }
    
    /// Descrambling
    fn descramble(&self, soft_bits: &[f32]) -> Vec<f32> {
        let mut descrambled = Vec::with_capacity(soft_bits.len());
        
        // Generate scrambling sequence - use N_id (Physical Cell ID)
        let c_init = self.pci.0 as u32;
        let scrambling_seq = self.generate_gold_sequence(c_init, soft_bits.len());
        
        // Apply descrambling
        for (i, &soft_bit) in soft_bits.iter().enumerate() {
            let sign = if scrambling_seq[i] == 0 { 1.0 } else { -1.0 };
            descrambled.push(soft_bit * sign);
        }
        
        descrambled
    }
    
    /// Rate de-matching
    fn rate_dematch(&self, soft_bits: &[f32]) -> Vec<f32> {
        // For simplified implementation, just return as-is
        soft_bits.to_vec()
    }
    
    /// Channel decoding using Polar decoder
    fn channel_decode(&self, soft_bits: &[f32]) -> Result<Vec<u8>, LayerError> {
        // TODO: Implement proper Polar decoder (successive cancellation or list decoding)
        // For now, use hard decision and simple decoding
        // This is a critical piece missing for proper PBCH reception
        
        debug!("PBCH Polar decoding: {} soft bits", soft_bits.len());
        
        let e = soft_bits.len();
        let k = Self::PBCH_PAYLOAD_SIZE; // 56
        
        // Create same Polar code structure as encoder
        let code = PolarCode::new(k, e, 9);
        let n = code.get_n();
        
        // For now, convert soft bits to hard bits
        let hard_bits: Vec<u8> = soft_bits.iter()
            .map(|&sb| if sb < 0.0 { 1 } else { 0 })
            .collect();
        
        // Rate de-matching (inverse of rate matching)
        let mut dematched = vec![0u8; n];
        if e >= n {
            // De-repetition: average repeated bits
            for i in 0..n {
                let mut count = 0;
                let mut sum = 0;
                for j in (i..e).step_by(n) {
                    sum += hard_bits[j] as i32;
                    count += 1;
                }
                dematched[i] = if sum > count / 2 { 1 } else { 0 };
            }
        } else {
            // De-shortening: fill with zeros
            dematched[..e].copy_from_slice(&hard_bits[..e]);
        }
        
        // TODO: Implement proper Polar decoding here
        // For now, just extract information bits from reliable positions
        let frozen_bits = code.get_frozen_bits();
        let mut decoded = Vec::with_capacity(k);
        
        for (i, &is_info) in frozen_bits.iter().enumerate() {
            if is_info && decoded.len() < k {
                decoded.push(dematched[i]);
            }
        }
        
        // Ensure we have exactly k bits
        decoded.resize(k, 0);
        
        debug!("PBCH decoded {} bits", decoded.len());
        Ok(decoded)
    }
    
    /// Check CRC
    fn check_crc(&self, bits: &[u8]) -> Result<Vec<u8>, LayerError> {
        if bits.len() != Self::PBCH_PAYLOAD_SIZE {
            return Err(LayerError::InvalidConfiguration(
                format!("Invalid PBCH payload size: {}", bits.len())
            ));
        }
        
        // Split A-bar and CRC
        let a_bar = &bits[..Self::PBCH_A_BAR_SIZE];
        let received_crc = &bits[Self::PBCH_A_BAR_SIZE..Self::PBCH_PAYLOAD_SIZE];
        
        // Calculate expected CRC
        let calculated_crc = self.calculate_crc24(a_bar);
        
        // Check CRC
        let mut crc_ok = true;
        for i in 0..24 {
            if received_crc[i] != ((calculated_crc >> (23 - i)) & 1) as u8 {
                crc_ok = false;
                break;
            }
        }
        
        if crc_ok {
            // Return only MIB bits (first 24 bits of A-bar)
            Ok(a_bar[..24].to_vec())
        } else {
            Err(LayerError::CrcFailed)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_mib_encoding() {
        let mib = Mib {
            sfn: 42,
            subcarrier_spacing_common: SubcarrierSpacing::Scs15,
            ssb_subcarrier_offset: 5,
            dmrs_type_a_position: DmrsPosition::Pos3,
            pdcch_config_sib1: 0x55,
            cell_barred: false,
            intra_freq_reselection: true,
            spare: 0,
        };
        
        let bits = mib.encode();
        assert_eq!(bits.len(), 24);
        
        let decoded = Mib::decode(&bits).unwrap();
        assert_eq!(decoded.sfn, mib.sfn);
        assert_eq!(decoded.ssb_subcarrier_offset, mib.ssb_subcarrier_offset);
        assert_eq!(decoded.pdcch_config_sib1, mib.pdcch_config_sib1);
    }
    
    #[test]
    fn test_pbch_processor() {
        let pci = Pci::new(123).unwrap();
        let cell_id = CellId(1);
        let processor = PbchProcessor::new(pci, cell_id).unwrap();
        
        let mib = processor.generate_mib(100);
        let encoded = processor.encode_pbch(&mib, 100);
        
        assert_eq!(encoded.len(), 432); // 864 bits / 2 (QPSK)
    }
}