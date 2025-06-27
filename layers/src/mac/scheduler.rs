//! MAC Scheduler Implementation
//! 
//! Handles scheduling of system information (SSB, SIB1) and user data

use crate::LayerError;
use common::types::{SubcarrierSpacing, Bandwidth, CellId};
use tracing::{debug, info};

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
    /// CORESET configuration (detailed)
    pub coreset: common::CorsetConfig,
    /// Frequency domain assignment for DCI
    pub frequency_domain_assignment: u16,
    /// Time domain assignment for DCI
    pub time_domain_assignment: u8,
    /// MCS index
    pub mcs_index: u8,
    /// Aggregation level for PDCCH
    pub aggregation_level: u8,
    /// CCE index
    pub cce_index: u16,
    /// Transport block size in bytes
    pub tbs_bytes: usize,
    /// Modulation scheme
    pub modulation: common::ModulationScheme,
    /// PRB allocation
    pub prb_allocation: Vec<u16>,
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
        coreset0_index: u8,
    ) -> Result<Self, LayerError> {
        // Get CORESET#0 configuration from MIB pdcch_config_sib1
        // Use the coreset0_index from configuration
        let coreset0_config = Coreset0Config::from_index(coreset0_index)?;
        
        Ok(Self {
            cell_id,
            scs,
            bandwidth,
            ssb_period_ms: 20,  // 20ms SSB periodicity for initial cell search
            sib1_period_ms: 20,  // 20ms SIB1 periodicity when SSB period <= 20ms (TS 38.331)
            coreset0_config,
        })
    }
    
    /// Get Type0-PDCCH CSS monitoring slots for SIB1
    /// Based on TS 38.213 Table 13-11
    pub fn get_sib1_monitoring_slots(&self) -> Vec<u32> {
        // For CORESET#0 index 6 with 15 kHz SCS:
        // From Table 13-11: O = 0, n_0 = 0
        // Monitoring slots are n_0 + i*M where M = 20 slots (20ms for 15 kHz)
        // Within SI window of 160ms = 160 slots
        let mut slots = Vec::new();
        let monitoring_period_slots = 20;  // 20 slots = 20ms for 15 kHz SCS
        let si_window_slots = 160;  // 160 slots = 160ms for 15 kHz SCS
        
        // Generate monitoring slots: 0, 20, 40, 60, 80, 100, 120, 140
        for i in 0..8 {
            let slot = i * monitoring_period_slots;
            if slot < si_window_slots {
                slots.push(slot);
            }
        }
        
        info!("SIB1 Type0-PDCCH monitoring slots within SI window: {:?}", slots);
        slots
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
            // For Case A (15 kHz SCS), determine which SSB based on slot
            // Slot 0: SSB#0 (symbol 2) and SSB#1 (symbol 8)
            // Slot 1: SSB#2 (symbol 2) and SSB#3 (symbol 8)
            // Note: MAC doesn't need to handle multiple SSBs per slot,
            // PHY will use frame_structure fallback for correct timing
            let (ssb_index, start_symbol) = match slot {
                0 => (0, 2),  // First SSB in slot 0 starts at symbol 2
                1 => (2, 2),  // First SSB in slot 1 starts at symbol 2
                _ => (0, 0),  // Should not happen for Case A
            };
            
            schedule.ssb_info = Some(SsbScheduleInfo {
                ssb_index,
                start_symbol,
            });
            debug!("Scheduled SSB #{} in frame={}, slot={}, start_symbol={}", 
                   ssb_index, frame, slot, start_symbol);
        }
        
        // Check if this slot should have SIB1
        if self.is_sib1_slot(frame, slot, slots_per_frame) {
            let total_slots = frame * slots_per_frame + slot as u32;
            // SIB1 is transmitted in slots following SSB
            // Use Type0-PDCCH CSS n0 configuration
            // Calculate PDCCH and PDSCH parameters for SIB1
            let prb_start = self.coreset0_config.rb_offset;
            let prb_length = 12; // Typical allocation for SIB1
            let tbs_bytes = 100; // Typical SIB1 size
            
            schedule.sib1_info = Some(Sib1ScheduleInfo {
                coreset0: self.coreset0_config.clone(),
                pdsch_time_alloc: PdschTimeAlloc {
                    start_symbol: self.coreset0_config.num_symbols as u8,  // After CORESET#0
                    num_symbols: 4,  // Typical allocation
                },
                payload_size: tbs_bytes,
                coreset: common::CorsetConfig {
                    start_symbol: 0,
                    duration: self.coreset0_config.num_symbols as u8,
                    frequency_domain_resources: (prb_start..prb_start + self.coreset0_config.num_rbs)
                        .map(|rb| rb as u16)
                        .collect(),
                },
                frequency_domain_assignment: ((prb_length * (prb_length + 1)) / 2 + prb_start) as u16,
                time_domain_assignment: 0,  // Row 0 in time domain allocation table
                mcs_index: 2,  // Conservative MCS for SIB1
                aggregation_level: 4,  // AL=4 for good coverage
                cce_index: 0,  // Start from CCE 0
                tbs_bytes,
                modulation: common::ModulationScheme::Qpsk,
                prb_allocation: (prb_start..prb_start + prb_length)
                    .map(|rb| rb as u16)
                    .collect(),
            });
            info!("Scheduled SIB1 Type0-PDCCH in frame={}, slot={} (slot {} in SI window)", 
                  frame, slot, total_slots % (160 / 10 * slots_per_frame));
        }
        
        schedule
    }
    
    /// Check if this slot should contain SSB
    fn is_ssb_slot(&self, frame: u32, slot: u8, _slots_per_frame: u32) -> bool {
        // SSB every 20ms (2 frames)
        let ssb_period_frames = self.ssb_period_ms / 10;
        let frame_in_period = frame % ssb_period_frames;
        
        // Transmit SSB in first frame of period, slots 0 and 1
        frame_in_period == 0 && (slot == 0 || slot == 1)
    }
    
    /// Check if this slot should contain SIB1
    fn is_sib1_slot(&self, frame: u32, slot: u8, slots_per_frame: u32) -> bool {
        // SIB1 scheduling follows TS 38.213 Table 13-11 for Type0-PDCCH CSS
        // For CORESET#0 index 6 with 15 kHz SCS: O = 0, n_0 = 0
        // Monitor slots n0 + i*M where M is the monitoring periodicity
        
        // When SSB period <= 20ms, SIB1 must be transmitted every 20ms (TS 38.331)
        let sib1_period_frames = self.sib1_period_ms / 10;  // 20ms = 2 frames
        let total_slots = frame * slots_per_frame + slot as u32;
        
        // Calculate slot offset within SI window (160ms)
        let si_window_slots = 160 / 10 * slots_per_frame;  // 160ms window in slots
        let slot_in_si_window = total_slots % si_window_slots;
        
        // Type0-PDCCH monitoring occasions for SIB1 (Table 13-11)
        // For 15 kHz SCS: n_0 = 0, monitor slots 0, 20, 40, ... within SI window
        // Since 20ms = 2 frames = 20 slots (for 15 kHz), monitor every 20 slots
        let monitoring_period_slots = 20;  // 20 slots = 20ms for 15 kHz SCS
        
        // Check if this is a Type0-PDCCH monitoring slot
        if slot_in_si_window % monitoring_period_slots == 0 {
            // PDCCH for SIB1 is transmitted in this slot
            // Note: PDSCH follows 2 slots later, but scheduler indicates PDCCH slot
            return true;
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
            6,  // CORESET#0 index 6
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
    
    #[test]
    fn test_sib1_scheduling() {
        let scheduler = MacScheduler::new(
            CellId(1),
            SubcarrierSpacing::Scs15,
            Bandwidth::Bw20,
            6,  // CORESET#0 index 6
        ).unwrap();
        
        // SIB1 should be scheduled every 20ms (20 slots for 15 kHz SCS)
        // Check first SI window (160ms = 16 frames = 160 slots)
        let mut sib1_slots = Vec::new();
        for frame in 0..16 {
            for slot in 0..10 {
                let schedule = scheduler.get_slot_schedule(frame, slot);
                if schedule.sib1_info.is_some() {
                    sib1_slots.push(frame * 10 + slot as u32);
                }
            }
        }
        
        // Expected SIB1 slots: 0, 20, 40, 60, 80, 100, 120, 140
        let expected_slots: Vec<u32> = vec![0, 20, 40, 60, 80, 100, 120, 140];
        assert_eq!(sib1_slots, expected_slots, 
                   "SIB1 should be scheduled every 20ms within SI window");
        
        // Verify monitoring slots match Table 13-11
        let monitoring_slots = scheduler.get_sib1_monitoring_slots();
        assert_eq!(monitoring_slots, expected_slots,
                   "Type0-PDCCH monitoring slots should match Table 13-11");
    }
}