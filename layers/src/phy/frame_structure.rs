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
        
        let ssb_period_frames = 2; // 20ms period = 2 frames
        let frame_in_period = frame % ssb_period_frames;
        
        // Only transmit SSB in even frames (every 20ms)
        if frame_in_period != 0 {
            return false;
        }
        
        // For Case A with 15 kHz SCS:
        // SSB #0: symbols 2-5 (in slot 0)
        // SSB #1: symbols 8-11 (in slot 0)  
        // SSB #2: symbols 16-19 (in slot 1)
        // SSB #3: symbols 22-25 (in slot 1)
        
        // SSB blocks span across slots 0 and 1 only for Case A
        matches!(slot, 0 | 1)
    }
    
    /// Check if this is a synchronization symbol
    pub fn is_sync_symbol(&self, frame: u32, slot: u8, symbol: u8) -> bool {
        // SSB periodicity check
        let ssb_period_frames = 2; // 20ms period = 2 frames
        let frame_in_period = frame % ssb_period_frames;
        if frame_in_period != 0 {
            return false;
        }
        
        // For Case A with 15 kHz SCS, check if this symbol is part of any SSB block
        match (slot, symbol) {
            // Slot 0: SSB #0 (symbols 2-5) and SSB #1 (symbols 8-11)
            (0, 2..=5) => true,   // SSB #0
            (0, 8..=11) => true,  // SSB #1
            // Slot 1: SSB #2 (symbols 2-5) and SSB #3 (symbols 8-11)
            (1, 2..=5) => true,   // SSB #2
            (1, 8..=11) => true,  // SSB #3
            _ => false,
        }
    }
    
    /// Check if this is a PSS symbol
    pub fn is_pss_symbol(&self, symbol: u8) -> bool {
        // PSS is the first symbol of each SSB block
        // For Case A: symbols 2, 8 within the slot
        matches!(symbol, 2 | 8)
    }
    
    /// Check if this is an SSS symbol
    pub fn is_sss_symbol(&self, symbol: u8) -> bool {
        // SSS is the third symbol of each SSB block (PSS + 2)
        // For Case A: symbols 4, 10 within the slot
        matches!(symbol, 4 | 10)
    }
    
    /// Check if this is a PBCH symbol
    pub fn is_pbch_symbol(&self, frame: u32, slot: u8, symbol: u8) -> bool {
        // SSB periodicity check
        let ssb_period_frames = 2; // 20ms period = 2 frames
        let frame_in_period = frame % ssb_period_frames;
        if frame_in_period != 0 {
            return false;
        }
        
        // PBCH is in the 2nd and 4th symbols of each SSB block
        // For Case A with 15 kHz SCS:
        match (slot, symbol) {
            // Slot 0: SSB #0 PBCH (symbols 3, 5) and SSB #1 PBCH (symbols 9, 11)
            (0, 3 | 5 | 9 | 11) => true,
            // Slot 1: SSB #2 PBCH (symbols 3, 5) and SSB #3 PBCH (symbols 9, 11)
            (1, 3 | 5 | 9 | 11) => true,
            _ => false,
        }
    }
    
    /// Get SSB index for given slot and symbol (Case A)
    /// Returns None if not an SSB symbol
    pub fn get_ssb_index(&self, slot: u8, symbol: u8) -> Option<u8> {
        match (slot, symbol) {
            // Slot 0: SSB #0 and SSB #1
            (0, 2..=5) => Some(0),   // SSB #0
            (0, 8..=11) => Some(1),  // SSB #1
            // Slot 1: SSB #2 and SSB #3
            (1, 2..=5) => Some(2),   // SSB #2
            (1, 8..=11) => Some(3),  // SSB #3
            _ => None,
        }
    }
    
    /// Get the first symbol of SSB block for given SSB index (Case A)
    pub fn get_ssb_start_symbol(&self, ssb_index: u8) -> Option<u8> {
        match ssb_index {
            0 => Some(2),   // SSB #0 starts at symbol 2
            1 => Some(8),   // SSB #1 starts at symbol 8
            2 => Some(2),   // SSB #2 starts at symbol 2 (in slot 1)
            3 => Some(8),   // SSB #3 starts at symbol 8 (in slot 1)
            _ => None,
        }
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
                // Normal CP: only first symbol of slot has longer CP
                if symbol_in_slot == 0 {
                    // Extended CP for first symbol - use ceiling for proper rounding
                    ((fft_size as f32 * 160.0 / 2048.0).ceil()) as usize
                } else {
                    // Normal CP for other symbols - use ceiling for proper rounding
                    ((fft_size as f32 * 144.0 / 2048.0).ceil()) as usize
                }
            }
            super::CyclicPrefix::Extended => {
                // Extended CP: all symbols have same CP length
                ((fft_size as f32 * 512.0 / 2048.0).ceil()) as usize
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