//! Physical Broadcast Channel (PBCH) Processing
//! 
//! Implements PBCH encoding/decoding according to 3GPP TS 38.212

use crate::LayerError;
use common::types::{Pci, CellId, SubcarrierSpacing};
use num_complex::Complex32;
use serde::{Serialize, Deserialize};

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
        
        let mut mib = Self::new();
        
        // System frame number (6 bits)
        mib.sfn = 0;
        for i in 0..6 {
            mib.sfn = (mib.sfn << 1) | (bits[i] as u16);
        }
        
        // Subcarrier spacing common (1 bit)
        mib.subcarrier_spacing_common = if bits[6] == 0 {
            SubcarrierSpacing::Scs15
        } else {
            SubcarrierSpacing::Scs30
        };
        
        // SSB subcarrier offset (4 bits)
        mib.ssb_subcarrier_offset = 0;
        for i in 7..11 {
            mib.ssb_subcarrier_offset = (mib.ssb_subcarrier_offset << 1) | bits[i];
        }
        
        // DMRS position (1 bit)
        mib.dmrs_type_a_position = if bits[11] == 0 {
            DmrsPosition::Pos2
        } else {
            DmrsPosition::Pos3
        };
        
        // PDCCH config SIB1 (8 bits)
        mib.pdcch_config_sib1 = 0;
        for i in 12..20 {
            mib.pdcch_config_sib1 = (mib.pdcch_config_sib1 << 1) | bits[i];
        }
        
        // Cell barred (1 bit)
        mib.cell_barred = bits[20] == 1;
        
        // Intra frequency reselection (1 bit)
        mib.intra_freq_reselection = bits[21] == 1;
        
        // Spare (1 bit)
        mib.spare = bits[22];
        
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
        
        // Set system frame number (only 6 MSBs)
        mib.sfn = ((frame_number >> 2) & 0x3F) as u16;
        
        // Configure based on cell parameters
        mib.subcarrier_spacing_common = SubcarrierSpacing::Scs15;
        mib.ssb_subcarrier_offset = 0;
        mib.dmrs_type_a_position = DmrsPosition::Pos2;
        
        // Configure PDCCH for SIB1
        // For Band 3 FDD with 15 kHz SCS, we use a typical configuration:
        // CORESET#0 table index 1: 24 RBs, 2 symbols, offset 2
        // SearchSpace#0 index 0: standard search space configuration
        // pdcch_config_sib1 = (coreset0_idx << 4) | searchspace0_idx
        mib.pdcch_config_sib1 = (1 << 4) | 0;  // CORESET#0 index 1, SearchSpace#0 index 0 = 0x10
        
        mib.cell_barred = false;
        mib.intra_freq_reselection = true;
        
        mib
    }
    
    /// Encode PBCH payload
    pub fn encode_pbch(&self, mib: &Mib, frame_number: u32) -> Vec<Complex32> {
        // Create complete BCH payload (A-bar)
        let mut a_bar = Vec::with_capacity(Self::PBCH_A_BAR_SIZE);
        
        // Add MIB bits (24 bits)
        let mib_bits = mib.encode();
        a_bar.extend_from_slice(&mib_bits);
        
        // Add additional PBCH payload (8 bits)
        // SFN bits 2-5 (4 bits)
        let sfn_bits = ((frame_number >> 2) & 0xF) as u8;
        for i in (0..4).rev() {
            a_bar.push(((sfn_bits >> i) & 1) as u8);
        }
        
        // Half-frame bit (1 bit) - bit 1 of SFN
        a_bar.push(((frame_number >> 1) & 1) as u8);
        
        // For L_max <= 8, add k_SSB (1 bit) and 2 reserved bits
        // k_SSB indicates SSB subcarrier offset (0 for k_SSB=0)
        a_bar.push(0); // k_SSB = 0
        a_bar.push(0); // Reserved
        a_bar.push(0); // Reserved
        
        // Add CRC-24
        let payload_with_crc = self.add_crc24(&a_bar);
        
        // Channel coding (simplified - should use Polar coding)
        let encoded_bits = self.channel_encode(&payload_with_crc);
        
        // Rate matching
        let rate_matched = self.rate_match(&encoded_bits);
        
        // Scrambling - use full SFN for scrambling
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
    
    /// Channel encoding (simplified - should use Polar code)
    fn channel_encode(&self, bits: &[u8]) -> Vec<u8> {
        // Simple repetition coding for demonstration
        let mut encoded = Vec::with_capacity(Self::PBCH_ENCODED_SIZE);
        
        // Repeat each bit multiple times
        let repetition = Self::PBCH_ENCODED_SIZE / bits.len();
        for &bit in bits {
            for _ in 0..repetition {
                encoded.push(bit);
            }
        }
        
        // Pad if necessary
        while encoded.len() < Self::PBCH_ENCODED_SIZE {
            encoded.push(0);
        }
        
        encoded
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
    
    /// Scrambling with Gold sequence
    fn scramble(&self, bits: &[u8], frame_number: u32) -> Vec<u8> {
        let mut scrambled = Vec::with_capacity(bits.len());
        
        // Initialize scrambling sequence according to 3GPP TS 38.211
        // c_init = cell_id for PBCH
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
        
        // Generate scrambling sequence - use cell ID
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
    
    /// Channel decoding (simplified)
    fn channel_decode(&self, soft_bits: &[f32]) -> Result<Vec<u8>, LayerError> {
        // For now, use simple repetition decoding
        // Real implementation should use Polar decoder
        let payload_size = Self::PBCH_PAYLOAD_SIZE;
        let repetition = soft_bits.len() / payload_size;
        
        if repetition == 0 {
            return Err(LayerError::InvalidConfiguration(
                "Invalid PBCH soft bits length".to_string()
            ));
        }
        
        let mut decoded = Vec::with_capacity(payload_size);
        
        for i in 0..payload_size {
            let mut sum = 0.0;
            for j in 0..repetition {
                if i + j * payload_size < soft_bits.len() {
                    sum += soft_bits[i + j * payload_size];
                }
            }
            decoded.push(if sum < 0.0 { 1 } else { 0 });
        }
        
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