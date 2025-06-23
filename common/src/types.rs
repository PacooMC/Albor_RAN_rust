//! Common Types for 5G GNodeB
//! 
//! Defines fundamental types used throughout the protocol stack

use serde::{Deserialize, Serialize};
use num_derive::{FromPrimitive, ToPrimitive};

/// Radio Network Temporary Identifier (RNTI)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Rnti(pub u16);

impl Rnti {
    /// Create a new RNTI
    pub fn new(value: u16) -> Self {
        Self(value)
    }
    
    /// Get the RNTI value
    pub fn value(&self) -> u16 {
        self.0
    }
}

/// Cell Identity
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CellId(pub u16);

/// Physical Cell Identity
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Pci(pub u16);

impl Pci {
    /// Maximum valid PCI value (0-1007)
    pub const MAX: u16 = 1007;
    
    /// Create a new PCI with validation
    pub fn new(value: u16) -> Option<Self> {
        if value <= Self::MAX {
            Some(Self(value))
        } else {
            None
        }
    }
}

/// Subcarrier spacing values in kHz
#[derive(Debug, Clone, Copy, PartialEq, Eq, FromPrimitive, ToPrimitive, Serialize, Deserialize)]
pub enum SubcarrierSpacing {
    /// 15 kHz
    Scs15 = 15,
    /// 30 kHz
    Scs30 = 30,
    /// 60 kHz
    Scs60 = 60,
    /// 120 kHz
    Scs120 = 120,
    /// 240 kHz
    Scs240 = 240,
}

/// Bandwidth values in MHz
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Bandwidth {
    /// 5 MHz
    Bw5,
    /// 10 MHz
    Bw10,
    /// 15 MHz
    Bw15,
    /// 20 MHz
    Bw20,
    /// 25 MHz
    Bw25,
    /// 30 MHz
    Bw30,
    /// 40 MHz
    Bw40,
    /// 50 MHz
    Bw50,
    /// 60 MHz
    Bw60,
    /// 80 MHz
    Bw80,
    /// 100 MHz
    Bw100,
}

impl Bandwidth {
    /// Get bandwidth in Hz
    pub fn as_hz(&self) -> u32 {
        match self {
            Bandwidth::Bw5 => 5_000_000,
            Bandwidth::Bw10 => 10_000_000,
            Bandwidth::Bw15 => 15_000_000,
            Bandwidth::Bw20 => 20_000_000,
            Bandwidth::Bw25 => 25_000_000,
            Bandwidth::Bw30 => 30_000_000,
            Bandwidth::Bw40 => 40_000_000,
            Bandwidth::Bw50 => 50_000_000,
            Bandwidth::Bw60 => 60_000_000,
            Bandwidth::Bw80 => 80_000_000,
            Bandwidth::Bw100 => 100_000_000,
        }
    }
    
    /// Get the sample rate for this bandwidth
    pub fn to_sample_rate(&self) -> f64 {
        // LTE/NR sample rates based on bandwidth
        match self {
            Bandwidth::Bw5 => 7.68e6,
            Bandwidth::Bw10 => 15.36e6,
            Bandwidth::Bw15 => 23.04e6,
            Bandwidth::Bw20 => 30.72e6,
            Bandwidth::Bw25 => 30.72e6,  // Uses same as 20 MHz
            Bandwidth::Bw30 => 46.08e6,
            Bandwidth::Bw40 => 61.44e6,
            Bandwidth::Bw50 => 61.44e6,  // Uses same as 40 MHz
            Bandwidth::Bw60 => 92.16e6,
            Bandwidth::Bw80 => 122.88e6,
            Bandwidth::Bw100 => 122.88e6, // Uses same as 80 MHz
        }
    }
}

/// Duplex mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DuplexMode {
    /// Frequency Division Duplex
    Fdd,
    /// Time Division Duplex
    Tdd,
}

/// QoS Class Identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Qci(pub u8);

impl Qci {
    /// Voice QCI
    pub const VOICE: Self = Self(1);
    /// Video QCI
    pub const VIDEO: Self = Self(2);
    /// Default bearer QCI
    pub const DEFAULT: Self = Self(9);
}

/// Tracking Area Code
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tac(pub u32);

/// PLMN Identity (MCC + MNC)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlmnId {
    /// Mobile Country Code
    pub mcc: [u8; 3],
    /// Mobile Network Code (2 or 3 digits)
    pub mnc: [u8; 3],
    /// MNC length (2 or 3)
    pub mnc_len: u8,
}

impl PlmnId {
    /// Create a new PLMN ID
    pub fn new(mcc: [u8; 3], mnc: [u8; 3], mnc_len: u8) -> Option<Self> {
        if mnc_len == 2 || mnc_len == 3 {
            Some(Self { mcc, mnc, mnc_len })
        } else {
            None
        }
    }
    
    /// Encode to 3-byte format used in 3GPP
    pub fn encode(&self) -> [u8; 3] {
        let mut encoded = [0u8; 3];
        encoded[0] = (self.mcc[1] << 4) | self.mcc[0];
        encoded[1] = if self.mnc_len == 2 {
            0xF0 | self.mcc[2]
        } else {
            (self.mnc[2] << 4) | self.mcc[2]
        };
        encoded[2] = (self.mnc[1] << 4) | self.mnc[0];
        encoded
    }
}

/// S-NSSAI (Single Network Slice Selection Assistance Information)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SNssai {
    /// Slice/Service Type
    pub sst: u8,
    /// Slice Differentiator (optional)
    pub sd: Option<u32>,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_pci_validation() {
        assert!(Pci::new(0).is_some());
        assert!(Pci::new(1007).is_some());
        assert!(Pci::new(1008).is_none());
    }
    
    #[test]
    fn test_bandwidth_conversion() {
        assert_eq!(Bandwidth::Bw20.as_hz(), 20_000_000);
        assert_eq!(Bandwidth::Bw100.as_hz(), 100_000_000);
    }
    
    #[test]
    fn test_plmn_encoding() {
        let plmn = PlmnId::new([2, 0, 8], [9, 3, 0], 2).unwrap();
        let encoded = plmn.encode();
        assert_eq!(encoded, [0x02, 0xF8, 0x39]);
    }
}