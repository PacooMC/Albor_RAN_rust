//! MAC Scheduler Implementation
//! 
//! Handles scheduling of system information (SSB, SIB1) and user data

use crate::LayerError;
use common::types::{SubcarrierSpacing, Bandwidth, CellId};
use tracing::{debug, info, trace};
use std::time::Duration;

/// CORESET#0 configuration based on 3GPP TS 38.213
#[derive(Debug, Clone)]
pub struct Coreset0Config {
    /// Number of resource blocks
    pub num_rbs: u32,
    /// Number of symbols
    pub num_symbols: u32,
    /// RB offset from point A
    pub rb_offset: u32,
}

impl Coreset0Config {
    /// Get CORESET#0 configuration from table index
    /// Based on 3GPP TS 38.213 Table 13-1 for {15, 15} kHz SCS
    pub fn from_index(index: u8) -> Result<Self, LayerError> {
        let config = match index {
            0 => Self { num_rbs: 24, num_symbols: 2, rb_offset: 0 },
            1 => Self { num_rbs: 24, num_symbols: 2, rb_offset: 2 },
            2 => Self { num_rbs: 24, num_symbols: 2, rb_offset: 4 },
            3 => Self { num_rbs: 24, num_symbols: 3, rb_offset: 0 },
            4 => Self { num_rbs: 24, num_symbols: 3, rb_offset: 2 },
            5 => Self { num_rbs: 24, num_symbols: 3, rb_offset: 4 },
            6 => Self { num_rbs: 48, num_symbols: 1, rb_offset: 12 },
            7 => Self { num_rbs: 48, num_symbols: 1, rb_offset: 16 },
            8 => Self { num_rbs: 48, num_symbols: 2, rb_offset: 12 },
            9 => Self { num_rbs: 48, num_symbols: 2, rb_offset: 16 },
            10 => Self { num_rbs: 48, num_symbols: 3, rb_offset: 12 },
            11 => Self { num_rbs: 48, num_symbols: 3, rb_offset: 16 },
            12 => Self { num_rbs: 96, num_symbols: 1, rb_offset: 38 },
            13 => Self { num_rbs: 96, num_symbols: 2, rb_offset: 38 },
            14 => Self { num_rbs: 96, num_symbols: 3, rb_offset: 38 },
            _ => return Err(LayerError::InvalidConfiguration(
                format!("Invalid CORESET#0 index: {}", index)
            )),
        };
        Ok(config)
    }
}

/// SearchSpace#0 configuration
#[derive(Debug, Clone)]
pub struct SearchSpace0Config {
    /// Monitoring slot periodicity
    pub slot_periodicity: u32,
    /// Slot offset
    pub slot_offset: u32,
    /// Number of PDCCH candidates
    pub num_candidates: u32,
}

/// Scheduling information for a slot
#[derive(Debug, Clone)]
pub struct SlotSchedule {
    /// Frame number
    pub frame: u32,
    /// Slot number
    pub slot: u8,
    /// SSB transmission info if scheduled
    pub ssb_info: Option<SsbScheduleInfo>,
    /// SIB1 transmission info if scheduled
    pub sib1_info: Option<Sib1ScheduleInfo>,
}

/// SSB scheduling information
#[derive(Debug, Clone)]
pub struct SsbScheduleInfo {
    /// SSB index (0-7 for FR1)
    pub ssb_index: u8,
    /// Starting symbol
    pub start_symbol: u8,
}

/// SIB1 scheduling information
#[derive(Debug, Clone)]
pub struct Sib1ScheduleInfo {
    /// CORESET#0 configuration
    pub coreset0: Coreset0Config,
    /// PDSCH time domain allocation
    pub pdsch_time_alloc: PdschTimeAlloc,
    /// SIB1 payload size in bytes
    pub payload_size: usize,
}

/// PDSCH time domain resource allocation
#[derive(Debug, Clone)]
pub struct PdschTimeAlloc {
    /// Starting symbol (K0 + S)
    pub start_symbol: u8,
    /// Number of symbols (L)
    pub num_symbols: u8,
}

/// MAC scheduler
pub struct MacScheduler {
    /// Cell ID
    cell_id: CellId,
    /// Subcarrier spacing
    scs: SubcarrierSpacing,
    /// Bandwidth
    bandwidth: Bandwidth,
    /// SSB periodicity in ms
    ssb_period_ms: u32,
    /// SIB1 periodicity in ms
    sib1_period_ms: u32,
    /// CORESET#0 configuration
    coreset0_config: Coreset0Config,
}

impl MacScheduler {
    /// Create a new MAC scheduler
    pub fn new(
        cell_id: CellId,
        scs: SubcarrierSpacing,
        bandwidth: Bandwidth,
    ) -> Result<Self, LayerError> {
        // Get CORESET#0 configuration from MIB pdcch_config_sib1
        // We configured it as index 1 in PBCH
        let coreset0_config = Coreset0Config::from_index(1)?;
        
        Ok(Self {
            cell_id,
            scs,
            bandwidth,
            ssb_period_ms: 20,  // 20ms SSB periodicity for initial cell search
            sib1_period_ms: 160, // 160ms SIB1 periodicity
            coreset0_config,
        })
    }
    
    /// Get schedule for a specific slot
    pub fn get_slot_schedule(&self, frame: u32, slot: u8) -> SlotSchedule {
        let mut schedule = SlotSchedule {
            frame,
            slot,
            ssb_info: None,
            sib1_info: None,
        };
        
        // Calculate timing based on SCS
        let slots_per_frame = match self.scs {
            SubcarrierSpacing::Scs15 => 10,
            SubcarrierSpacing::Scs30 => 20,
            SubcarrierSpacing::Scs60 => 40,
            SubcarrierSpacing::Scs120 => 80,
            SubcarrierSpacing::Scs240 => 160,
        };
        
        // Check if this slot should have SSB
        if self.is_ssb_slot(frame, slot, slots_per_frame) {
            schedule.ssb_info = Some(SsbScheduleInfo {
                ssb_index: 0,  // Single SSB beam for now
                start_symbol: 0,
            });
            debug!("Scheduled SSB in frame={}, slot={}", frame, slot);
        }
        
        // Check if this slot should have SIB1
        if self.is_sib1_slot(frame, slot, slots_per_frame) {
            // SIB1 is transmitted in slots following SSB
            // Use Type0-PDCCH CSS n0 configuration
            schedule.sib1_info = Some(Sib1ScheduleInfo {
                coreset0: self.coreset0_config.clone(),
                pdsch_time_alloc: PdschTimeAlloc {
                    start_symbol: self.coreset0_config.num_symbols as u8,  // After CORESET#0
                    num_symbols: 4,  // Typical allocation
                },
                payload_size: 100,  // Typical SIB1 size
            });
            info!("Scheduled SIB1 in frame={}, slot={}", frame, slot);
        }
        
        schedule
    }
    
    /// Check if this slot should contain SSB
    fn is_ssb_slot(&self, frame: u32, slot: u8, slots_per_frame: u32) -> bool {
        // SSB every 20ms (2 frames)
        let ssb_period_frames = self.ssb_period_ms / 10;
        let frame_in_period = frame % ssb_period_frames;
        
        // Transmit SSB in first frame of period, slot 0
        frame_in_period == 0 && slot == 0
    }
    
    /// Check if this slot should contain SIB1
    fn is_sib1_slot(&self, frame: u32, slot: u8, slots_per_frame: u32) -> bool {
        // SIB1 every 160ms (16 frames)
        let sib1_period_frames = self.sib1_period_ms / 10;
        let total_slots = frame * slots_per_frame + slot as u32;
        let sib1_period_slots = sib1_period_frames * slots_per_frame;
        
        // SIB1 is transmitted 2 slots after SSB in the same frame
        // This gives time for UE to decode MIB and prepare for SIB1 reception
        let slot_in_period = total_slots % sib1_period_slots;
        
        // Check if this is 2 slots after an SSB slot
        if slot_in_period == 2 {
            // Verify this is in an SSB frame
            let frame_in_period = (total_slots / slots_per_frame) % sib1_period_frames;
            return frame_in_period == 0;
        }
        
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_coreset0_config() {
        // Test valid index
        let config = Coreset0Config::from_index(1).unwrap();
        assert_eq!(config.num_rbs, 24);
        assert_eq!(config.num_symbols, 2);
        assert_eq!(config.rb_offset, 2);
        
        // Test invalid index
        assert!(Coreset0Config::from_index(20).is_err());
    }
    
    #[test]
    fn test_scheduler_ssb_timing() {
        let scheduler = MacScheduler::new(
            CellId(1),
            SubcarrierSpacing::Scs15,
            Bandwidth::Bw20,
        ).unwrap();
        
        // SSB should be in frame 0, slot 0
        let schedule = scheduler.get_slot_schedule(0, 0);
        assert!(schedule.ssb_info.is_some());
        
        // No SSB in frame 1
        let schedule = scheduler.get_slot_schedule(1, 0);
        assert!(schedule.ssb_info.is_none());
        
        // SSB again in frame 2 (20ms later)
        let schedule = scheduler.get_slot_schedule(2, 0);
        assert!(schedule.ssb_info.is_some());
    }
}