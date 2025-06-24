//! 5G NR Frame Structure Implementation
//! 
//! Implements the frame structure according to 3GPP TS 38.211

use common::types::SubcarrierSpacing;
use std::time::Duration;

/// Slot configuration based on numerology
#[derive(Debug, Clone)]
pub struct SlotConfig {
    /// Subcarrier spacing
    pub scs: SubcarrierSpacing,
    /// Number of slots per subframe (1ms)
    pub slots_per_subframe: u8,
    /// Number of slots per frame (10ms)
    pub slots_per_frame: u8,
    /// Number of OFDM symbols per slot
    pub symbols_per_slot: u8,
    /// Slot duration in microseconds
    pub slot_duration_us: u32,
}

impl SlotConfig {
    /// Create slot configuration from subcarrier spacing
    pub fn from_scs(scs: SubcarrierSpacing, extended_cp: bool) -> Self {
        let (slots_per_subframe, slot_duration_us) = match scs {
            SubcarrierSpacing::Scs15 => (1, 1000),
            SubcarrierSpacing::Scs30 => (2, 500),
            SubcarrierSpacing::Scs60 => (4, 250),
            SubcarrierSpacing::Scs120 => (8, 125),
            SubcarrierSpacing::Scs240 => (16, 62), // Actually 62.5 us
        };
        
        let symbols_per_slot = if extended_cp { 12 } else { 14 };
        
        Self {
            scs,
            slots_per_subframe,
            slots_per_frame: slots_per_subframe * 10,
            symbols_per_slot,
            slot_duration_us,
        }
    }
}

/// Symbol type in a slot
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolType {
    /// Downlink symbol
    Downlink,
    /// Uplink symbol
    Uplink,
    /// Flexible symbol (can be DL or UL)
    Flexible,
    /// Guard period
    Guard,
}

/// Frame structure for 5G NR
#[derive(Debug, Clone)]
pub struct FrameStructure {
    /// Slot configuration
    slot_config: SlotConfig,
    /// Cyclic prefix type
    cyclic_prefix: super::CyclicPrefix,
    /// Duplex mode
    duplex_mode: super::DuplexMode,
}

impl FrameStructure {
    /// Create a new frame structure
    pub fn new(
        scs: SubcarrierSpacing,
        cyclic_prefix: super::CyclicPrefix,
    ) -> Self {
        let extended_cp = matches!(cyclic_prefix, super::CyclicPrefix::Extended);
        let slot_config = SlotConfig::from_scs(scs, extended_cp);
        
        Self {
            slot_config,
            cyclic_prefix,
            duplex_mode: super::DuplexMode::Fdd, // Default to FDD
        }
    }
    
    /// Set duplex mode
    pub fn set_duplex_mode(&mut self, mode: super::DuplexMode) {
        self.duplex_mode = mode;
    }
    
    /// Get number of slots per frame
    pub fn slots_per_frame(&self) -> u8 {
        self.slot_config.slots_per_frame
    }
    
    /// Get number of symbols per slot
    pub fn symbols_per_slot(&self) -> u8 {
        self.slot_config.symbols_per_slot
    }
    
    /// Get slot duration
    pub fn slot_duration(&self) -> Duration {
        Duration::from_micros(self.slot_config.slot_duration_us as u64)
    }
    
    /// Get symbol duration
    pub fn symbol_duration(&self) -> Duration {
        Duration::from_micros(
            self.slot_config.slot_duration_us as u64 / self.slot_config.symbols_per_slot as u64
        )
    }
    
    /// Check if this is a synchronization slot
    pub fn is_sync_slot(&self, frame: u32, slot: u8) -> bool {
        // SSB periodicity: typically 20ms for initial cell search
        // This means SSB is transmitted every 2 frames (20ms)
        // For 15kHz SCS: 10 slots per frame, so SSB every 20 slots
        // For 30kHz SCS: 20 slots per frame, so SSB every 40 slots
        
        let ssb_period_frames = 2; // 20ms period = 2 frames
        let frame_in_period = frame % ssb_period_frames;
        
        // For band 3 (< 3 GHz), this is Case A
        // SSB can be transmitted in slots 0, 1, 2, 3 within the SSB period
        // We'll transmit SSB in all 4 slots for better detection
        frame_in_period == 0 && slot < 4
    }
    
    /// Check if this is a synchronization symbol
    pub fn is_sync_symbol(&self, frame: u32, slot: u8, symbol: u8) -> bool {
        if !self.is_sync_slot(frame, slot) {
            return false;
        }
        
        // PSS is in symbol 0, SSS is in symbol 2, PBCH is in symbols 1, 3
        matches!(symbol, 0..=3)
    }
    
    /// Check if this is a PSS symbol
    pub fn is_pss_symbol(&self, symbol: u8) -> bool {
        symbol == 0
    }
    
    /// Check if this is an SSS symbol
    pub fn is_sss_symbol(&self, symbol: u8) -> bool {
        symbol == 2
    }
    
    /// Check if this is a PBCH symbol
    pub fn is_pbch_symbol(&self, frame: u32, slot: u8, symbol: u8) -> bool {
        if !self.is_sync_slot(frame, slot) {
            return false;
        }
        
        // PBCH is in symbols 1 and 3
        matches!(symbol, 1 | 3)
    }
    
    /// Get symbol type for TDD
    pub fn get_symbol_type(&self, slot: u8, symbol: u8) -> SymbolType {
        match self.duplex_mode {
            super::DuplexMode::Fdd => SymbolType::Downlink, // FDD is always DL for DL carrier
            super::DuplexMode::Tdd { pattern } => {
                // Simple TDD pattern implementation
                // This should be enhanced with proper TDD configuration
                if slot < pattern.dl_slots {
                    SymbolType::Downlink
                } else if slot < (pattern.dl_slots + pattern.special_slots) {
                    // Special slot: first symbols DL, last symbols UL, middle is guard
                    if symbol < 10 {
                        SymbolType::Downlink
                    } else if symbol >= 12 {
                        SymbolType::Uplink
                    } else {
                        SymbolType::Guard
                    }
                } else if slot < (pattern.dl_slots + pattern.special_slots + pattern.ul_slots) {
                    SymbolType::Uplink
                } else {
                    SymbolType::Flexible
                }
            }
        }
    }
    
    /// Calculate samples per symbol including cyclic prefix
    pub fn samples_per_symbol(&self, fft_size: usize, symbol_in_slot: u8) -> usize {
        let base_cp = match self.cyclic_prefix {
            super::CyclicPrefix::Normal => {
                // Normal CP: first symbol has longer CP
                if symbol_in_slot == 0 || symbol_in_slot == 7 {
                    match self.slot_config.scs {
                        SubcarrierSpacing::Scs15 => (fft_size * 160) / 2048,
                        SubcarrierSpacing::Scs30 => (fft_size * 160) / 2048,
                        SubcarrierSpacing::Scs60 => (fft_size * 160) / 2048,
                        _ => (fft_size * 144) / 2048,
                    }
                } else {
                    (fft_size * 144) / 2048
                }
            }
            super::CyclicPrefix::Extended => {
                // Extended CP: all symbols have same CP length
                (fft_size * 512) / 2048
            }
        };
        
        fft_size + base_cp
    }
    
    /// Get SSB periodicity in milliseconds
    pub fn ssb_periodicity_ms(&self) -> u32 {
        // Default SSB periodicity is 20ms
        // This can be configured based on network requirements
        20
    }
    
    /// Check if current timing is for SSB transmission
    pub fn is_ssb_occasion(&self, frame: u32, slot: u8, symbol: u8) -> bool {
        // SSB transmitted every 20ms (2 frames)
        if frame % 2 != 0 {
            return false;
        }
        
        // Check if it's an SSB slot and symbol
        self.is_sync_symbol(frame, slot, symbol)
    }
}

/// SSB (Synchronization Signal Block) pattern
#[derive(Debug, Clone, Copy)]
pub struct SsbPattern {
    /// Case A, B, C, D, or E based on frequency range
    pub case: SsbCase,
    /// Maximum number of SSB beams
    pub max_beams: u8,
    /// SSB symbols within slot
    pub symbols: [u8; 4],
}

/// SSB cases based on frequency range
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SsbCase {
    /// Case A: f <= 3 GHz, 15 kHz SCS
    CaseA,
    /// Case B: 3 GHz < f <= 6 GHz, 30 kHz SCS
    CaseB,
    /// Case C: 3 GHz < f <= 6 GHz, 30 kHz SCS (paired spectrum)
    CaseC,
    /// Case D: f > 6 GHz, 120 kHz SCS
    CaseD,
    /// Case E: f > 6 GHz, 240 kHz SCS
    CaseE,
}

impl SsbPattern {
    /// Create SSB pattern based on frequency and SCS
    pub fn new(frequency_ghz: f32, scs: SubcarrierSpacing) -> Self {
        let (case, max_beams) = if frequency_ghz <= 3.0 {
            (SsbCase::CaseA, 4)
        } else if frequency_ghz <= 6.0 {
            match scs {
                SubcarrierSpacing::Scs15 => (SsbCase::CaseA, 4),
                SubcarrierSpacing::Scs30 => (SsbCase::CaseB, 8),
                _ => (SsbCase::CaseC, 8),
            }
        } else {
            match scs {
                SubcarrierSpacing::Scs120 => (SsbCase::CaseD, 64),
                SubcarrierSpacing::Scs240 => (SsbCase::CaseE, 64),
                _ => (SsbCase::CaseD, 64),
            }
        };
        
        // SSB occupies 4 consecutive symbols
        let symbols = [0, 1, 2, 3];
        
        Self {
            case,
            max_beams,
            symbols,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_slot_config() {
        let config = SlotConfig::from_scs(SubcarrierSpacing::Scs15, false);
        assert_eq!(config.slots_per_subframe, 1);
        assert_eq!(config.slots_per_frame, 10);
        assert_eq!(config.symbols_per_slot, 14);
        
        let config = SlotConfig::from_scs(SubcarrierSpacing::Scs30, false);
        assert_eq!(config.slots_per_subframe, 2);
        assert_eq!(config.slots_per_frame, 20);
    }
    
    #[test]
    fn test_frame_structure() {
        let fs = FrameStructure::new(
            SubcarrierSpacing::Scs15,
            super::super::CyclicPrefix::Normal,
        );
        
        assert_eq!(fs.slots_per_frame(), 10);
        assert_eq!(fs.symbols_per_slot(), 14);
        
        // Test sync detection
        assert!(fs.is_sync_slot(0, 0));
        assert!(fs.is_sync_slot(0, 1));
        assert!(fs.is_sync_slot(0, 2));
        assert!(fs.is_sync_slot(0, 3));
        assert!(!fs.is_sync_slot(0, 4));
        assert!(!fs.is_sync_slot(0, 5));
        assert!(!fs.is_sync_slot(1, 0)); // Frame 1 should not have SSB
        
        assert!(fs.is_pss_symbol(0));
        assert!(fs.is_sss_symbol(2));
    }
    
    #[test]
    fn test_ssb_pattern() {
        let pattern = SsbPattern::new(2.5, SubcarrierSpacing::Scs15);
        assert_eq!(pattern.case, SsbCase::CaseA);
        assert_eq!(pattern.max_beams, 4);
        
        let pattern = SsbPattern::new(3.5, SubcarrierSpacing::Scs30);
        assert_eq!(pattern.case, SsbCase::CaseB);
        assert_eq!(pattern.max_beams, 8);
    }
}