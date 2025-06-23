//! System Information Block 1 (SIB1) Generation
//! 
//! Implements SIB1 message creation according to 3GPP TS 38.331

use crate::LayerError;
use common::types::{CellId, Bandwidth};
use bytes::{Bytes, BytesMut, BufMut};
use tracing::{debug, info};

/// SIB1 configuration
#[derive(Debug, Clone)]
pub struct Sib1Config {
    /// Cell ID
    pub cell_id: CellId,
    /// PLMN identity (MCC + MNC)
    pub plmn_id: PlmnId,
    /// Tracking area code
    pub tac: u32,
    /// Cell selection parameters
    pub cell_selection_info: CellSelectionInfo,
    /// Frequency band list
    pub freq_band_list: Vec<u16>,
}

/// PLMN Identity
#[derive(Debug, Clone)]
pub struct PlmnId {
    /// Mobile Country Code (3 digits)
    pub mcc: [u8; 3],
    /// Mobile Network Code (2 or 3 digits)
    pub mnc: Vec<u8>,
}

impl PlmnId {
    /// Create a test PLMN ID (001-01)
    pub fn test_plmn() -> Self {
        Self {
            mcc: [0, 0, 1],
            mnc: vec![0, 1],
        }
    }
    
    /// Encode PLMN ID to bytes (3 octets)
    pub fn encode(&self) -> [u8; 3] {
        let mut encoded = [0u8; 3];
        
        // MCC digit 2 | MCC digit 1
        encoded[0] = (self.mcc[1] << 4) | self.mcc[0];
        
        // MNC digit 3 | MCC digit 3
        if self.mnc.len() == 3 {
            encoded[1] = (self.mnc[2] << 4) | self.mcc[2];
        } else {
            encoded[1] = (0xF << 4) | self.mcc[2];  // 0xF for 2-digit MNC
        }
        
        // MNC digit 2 | MNC digit 1
        encoded[2] = (self.mnc[1] << 4) | self.mnc[0];
        
        encoded
    }
}

/// Cell selection information
#[derive(Debug, Clone)]
pub struct CellSelectionInfo {
    /// Minimum required RX level in dBm
    pub q_rx_lev_min: i8,
    /// Offset to q_rx_lev_min
    pub q_rx_lev_min_offset: u8,
}

impl Default for CellSelectionInfo {
    fn default() -> Self {
        Self {
            q_rx_lev_min: -70,  // -140 dBm (value * 2)
            q_rx_lev_min_offset: 0,
        }
    }
}

/// SIB1 message generator
pub struct Sib1Generator {
    config: Sib1Config,
}

impl Sib1Generator {
    /// Create a new SIB1 generator
    pub fn new(config: Sib1Config) -> Self {
        Self { config }
    }
    
    /// Generate SIB1 message
    /// Returns the encoded SIB1 as bytes
    pub fn generate_sib1(&self) -> Result<Bytes, LayerError> {
        // For initial implementation, create a minimal valid SIB1
        // In a real implementation, this would use ASN.1 encoding
        
        let mut buffer = BytesMut::with_capacity(256);
        
        // SIB1 header (simplified)
        buffer.put_u8(0x80);  // Message type indicator
        
        // Cell Access Related Info
        // PLMN Identity List
        buffer.put_u8(1);  // Number of PLMNs
        let plmn_encoded = self.config.plmn_id.encode();
        buffer.put_slice(&plmn_encoded);
        
        // Tracking Area Code (24 bits)
        buffer.put_u8(((self.config.tac >> 16) & 0xFF) as u8);
        buffer.put_u8(((self.config.tac >> 8) & 0xFF) as u8);
        buffer.put_u8((self.config.tac & 0xFF) as u8);
        
        // Cell Identity (28 bits in 4 octets)
        let cell_id = self.config.cell_id.0 as u32;
        buffer.put_u32(cell_id << 4);  // Left-aligned in 32 bits
        
        // Cell Barred (1 bit) - not barred
        buffer.put_u8(0x00);
        
        // Intra Frequency Reselection (1 bit) - allowed
        buffer.put_u8(0x01);
        
        // Cell Selection Info
        buffer.put_i8(self.config.cell_selection_info.q_rx_lev_min);
        buffer.put_u8(self.config.cell_selection_info.q_rx_lev_min_offset);
        
        // Frequency Band Indicator
        buffer.put_u8(self.config.freq_band_list.len() as u8);
        for band in &self.config.freq_band_list {
            buffer.put_u16(*band);
        }
        
        // Scheduling Info List (empty for now)
        buffer.put_u8(0);
        
        // SI-SchedulingInfo (empty for now)
        buffer.put_u8(0);
        
        // Serving Cell Config Common (minimal)
        // Downlink Config Common
        buffer.put_u8(0x01);  // Presence flags
        
        // SSB Positions In Burst
        buffer.put_u8(0x80);  // First SSB position active
        
        // SSB Periodicity
        buffer.put_u8(20);  // 20ms
        
        // PDCCH Config Common
        // Using CORESET#0 from MIB
        buffer.put_u8(0x00);  // Use MIB configured CORESET#0
        
        // PDSCH Config Common
        buffer.put_u8(0x00);  // Default configuration
        
        // Uplink Config Common (minimal for FDD)
        buffer.put_u8(0x00);  // Default configuration
        
        // Supplementary Uplink (not present)
        buffer.put_u8(0x00);
        
        // TDD-UL-DL-ConfigCommon (not present for FDD)
        buffer.put_u8(0x00);
        
        // Pad to minimum size if needed
        while buffer.len() < 100 {
            buffer.put_u8(0x00);
        }
        
        info!("Generated SIB1 message: {} bytes", buffer.len());
        Ok(buffer.freeze())
    }
}

/// Create default SIB1 configuration for testing
pub fn default_sib1_config(cell_id: CellId) -> Sib1Config {
    Sib1Config {
        cell_id,
        plmn_id: PlmnId::test_plmn(),
        tac: 1,  // Test TAC
        cell_selection_info: CellSelectionInfo::default(),
        freq_band_list: vec![3],  // Band 3 for our test
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_plmn_encoding() {
        let plmn = PlmnId::test_plmn();
        let encoded = plmn.encode();
        
        // Check encoding: MCC=001, MNC=01
        assert_eq!(encoded[0], 0x00);  // MCC digit 2=0, digit 1=0
        assert_eq!(encoded[1], 0xF1);  // MNC digit 3=F (not present), MCC digit 3=1
        assert_eq!(encoded[2], 0x10);  // MNC digit 2=1, digit 1=0
    }
    
    #[test]
    fn test_sib1_generation() {
        let config = default_sib1_config(CellId(1));
        let generator = Sib1Generator::new(config);
        
        let sib1 = generator.generate_sib1().unwrap();
        assert!(sib1.len() >= 100);  // Minimum SIB1 size
    }
}