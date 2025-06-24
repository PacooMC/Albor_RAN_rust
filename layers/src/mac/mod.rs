//! Medium Access Control (MAC) Layer Implementation
//! 
//! Implements the 5G NR MAC layer according to 3GPP TS 38.321

pub mod scheduler;
pub mod sib1;

use crate::{LayerError, ProtocolLayer};
use crate::rrc::{RrcMacInterface, RrcMessageType, RarGrant};
use async_trait::async_trait;
use bytes::{Bytes, BytesMut, BufMut};
use tracing::{debug, info, warn, error};
use std::sync::Arc;
use std::sync::atomic::{AtomicU16, Ordering};
use tokio::sync::{Mutex, RwLock, mpsc};

pub use scheduler::{MacScheduler, SlotSchedule, SsbScheduleInfo, Sib1ScheduleInfo};
pub use sib1::{Sib1Generator, Sib1Config, default_sib1_config};
use common::types::{CellId, SubcarrierSpacing, Bandwidth, Rnti};

/// MAC PDU types
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MacPduType {
    /// Random Access Response
    Rar,
    /// Uplink/Downlink data
    Data,
    /// Broadcast (SI-RNTI)
    Broadcast,
}

/// Random Access procedure state
#[derive(Debug)]
struct RandomAccessProcedure {
    /// Temporary C-RNTI allocated
    tc_rnti: Rnti,
    /// Timing advance
    timing_advance: u16,
    /// Frame where PRACH was detected
    prach_frame: u32,
    /// Slot where PRACH was detected
    prach_slot: u8,
    /// Preamble index
    preamble_index: u8,
}

/// MAC layer configuration
#[derive(Debug, Clone)]
pub struct MacConfig {
    /// Cell ID
    pub cell_id: CellId,
    /// Subcarrier spacing
    pub scs: SubcarrierSpacing,
    /// Bandwidth
    pub bandwidth: Bandwidth,
    /// Maximum number of UEs
    pub max_ues: u16,
    /// SIB1 configuration
    pub sib1_config: Sib1Config,
}

/// MAC-PHY interface for scheduling information
#[async_trait]
pub trait MacPhyInterface: Send + Sync {
    /// Get scheduling information for a slot
    async fn get_slot_schedule(&self, frame: u32, slot: u8) -> Result<SlotSchedule, LayerError>;
    
    /// Get SIB1 payload
    async fn get_sib1_payload(&self) -> Result<Bytes, LayerError>;
    
    /// Report PRACH detection from PHY
    async fn report_prach_detection(&self, detection: crate::phy::prach::PrachDetectionResult) -> Result<(), LayerError>;
}

/// Enhanced MAC layer implementation
pub struct EnhancedMacLayer {
    config: MacConfig,
    scheduler: Arc<Mutex<MacScheduler>>,
    sib1_generator: Arc<Sib1Generator>,
    sib1_payload: Arc<RwLock<Option<Bytes>>>,
    initialized: bool,
    /// Next C-RNTI to allocate
    next_c_rnti: Arc<AtomicU16>,
    /// Ongoing Random Access procedures
    ra_procedures: Arc<Mutex<Vec<RandomAccessProcedure>>>,
    /// RRC message sender
    rrc_tx: Option<mpsc::Sender<(Rnti, Bytes)>>,
}

impl EnhancedMacLayer {
    /// Create a new enhanced MAC layer instance
    pub fn new(config: MacConfig) -> Result<Self, LayerError> {
        let scheduler = MacScheduler::new(
            config.cell_id,
            config.scs,
            config.bandwidth,
        )?;
        
        let sib1_generator = Sib1Generator::new(config.sib1_config.clone());
        
        Ok(Self {
            config,
            scheduler: Arc::new(Mutex::new(scheduler)),
            sib1_generator: Arc::new(sib1_generator),
            sib1_payload: Arc::new(RwLock::new(None)),
            initialized: false,
            next_c_rnti: Arc::new(AtomicU16::new(0x4601)), // Start C-RNTI allocation
            ra_procedures: Arc::new(Mutex::new(Vec::new())),
            rrc_tx: None,
        })
    }
    
    /// Set RRC message channel
    pub fn set_rrc_channel(&mut self, tx: mpsc::Sender<(Rnti, Bytes)>) {
        self.rrc_tx = Some(tx);
    }
    
    /// Generate Random Access Response
    fn generate_rar(&self, tc_rnti: Rnti, timing_advance: u16) -> Bytes {
        let mut buf = BytesMut::new();
        
        // MAC subheader (E/T/RAPID)
        buf.put_u8(0x40); // E=0, T=1, RAPID=0
        
        // MAC RAR (7 bytes)
        // Timing Advance Command (12 bits)
        let ta_high = ((timing_advance >> 4) & 0xFF) as u8;
        let ta_low = ((timing_advance & 0x0F) << 4) as u8;
        
        buf.put_u8(ta_high);
        
        // UL Grant (20 bits) - simplified
        buf.put_u8(ta_low | 0x0F); // TA low + grant high
        buf.put_u16(0xFFFF); // Rest of grant
        
        // Temporary C-RNTI
        buf.put_u16(tc_rnti.0);
        
        buf.freeze()
    }
    
    /// Process Msg3 (contains RRC Setup Request)
    async fn process_msg3(&self, tc_rnti: Rnti, data: Bytes) -> Result<(), LayerError> {
        info!("Processing Msg3 from TC-RNTI {}: {} bytes", tc_rnti.0, data.len());
        
        // Extract RRC message from MAC PDU
        // In real implementation, would parse MAC header
        // For now, assume entire payload is RRC message
        
        if let Some(rrc_tx) = &self.rrc_tx {
            // Forward to RRC layer
            if let Err(e) = rrc_tx.send((tc_rnti, data)).await {
                error!("Failed to send message to RRC: {}", e);
                return Err(LayerError::ProcessingError("RRC channel error".into()));
            }
            info!("Forwarded Msg3 to RRC layer");
        } else {
            warn!("No RRC channel configured");
        }
        
        Ok(())
    }
}

#[async_trait]
impl ProtocolLayer for EnhancedMacLayer {
    async fn initialize(&mut self) -> Result<(), LayerError> {
        info!("Initializing enhanced MAC layer");
        debug!("MAC config: cell_id={}, scs={:?}, bandwidth={:?}", 
               self.config.cell_id.0, 
               self.config.scs,
               self.config.bandwidth);
        
        // Generate SIB1 payload
        let sib1_payload = self.sib1_generator.generate_sib1()?;
        *self.sib1_payload.write().await = Some(sib1_payload);
        info!("Generated SIB1 payload");
        
        self.initialized = true;
        info!("Enhanced MAC layer initialized successfully");
        Ok(())
    }
    
    async fn process_uplink(&mut self, data: Bytes) -> Result<Bytes, LayerError> {
        if !self.initialized {
            return Err(LayerError::NotInitialized);
        }
        
        debug!("MAC processing uplink data: {} bytes", data.len());
        
        // Check if this is Msg3 from a Random Access procedure
        // In real implementation, this would be indicated by PHY
        let ra_procs = self.ra_procedures.lock().await;
        if !ra_procs.is_empty() {
            // Assume this is Msg3 from the first RA procedure
            let tc_rnti = ra_procs[0].tc_rnti;
            drop(ra_procs);
            
            // Process as Msg3
            self.process_msg3(tc_rnti, data.clone()).await?;
        }
        
        Ok(data)
    }
    
    async fn process_downlink(&mut self, data: Bytes) -> Result<Bytes, LayerError> {
        if !self.initialized {
            return Err(LayerError::NotInitialized);
        }
        
        debug!("MAC processing downlink data: {} bytes", data.len());
        
        // TODO: Implement MAC downlink processing
        // - Multiplex logical channels
        // - Add MAC headers
        // - Generate MAC control elements
        // - Create MAC PDUs
        
        Ok(data)
    }
    
    async fn shutdown(&mut self) -> Result<(), LayerError> {
        info!("Shutting down enhanced MAC layer");
        self.initialized = false;
        Ok(())
    }
}

#[async_trait]
impl MacPhyInterface for EnhancedMacLayer {
    async fn get_slot_schedule(&self, frame: u32, slot: u8) -> Result<SlotSchedule, LayerError> {
        if !self.initialized {
            return Err(LayerError::NotInitialized);
        }
        
        let scheduler = self.scheduler.lock().await;
        let schedule = scheduler.get_slot_schedule(frame, slot);
        
        Ok(schedule)
    }
    
    async fn get_sib1_payload(&self) -> Result<Bytes, LayerError> {
        if !self.initialized {
            return Err(LayerError::NotInitialized);
        }
        
        let payload = self.sib1_payload.read().await;
        payload.clone().ok_or_else(|| LayerError::InvalidState("SIB1 payload not generated".into()))
    }
    
    async fn report_prach_detection(&self, detection: crate::phy::prach::PrachDetectionResult) -> Result<(), LayerError> {
        if !self.initialized {
            return Err(LayerError::NotInitialized);
        }
        
        info!("PRACH detection reported: frame={}, slot={}, {} preambles detected", 
              detection.frame, detection.slot, detection.preambles.len());
        
        // Process each detected preamble
        for preamble in &detection.preambles {
            info!("  Preamble {}: TA={:.1}us, metric={:.2}, power={:.1}dBm",
                  preamble.preamble_index, 
                  preamble.timing_advance_us,
                  preamble.detection_metric,
                  preamble.power_dbm);
            
            // Initiate Random Access procedure
            // 1. Allocate TC-RNTI for the UE
            let tc_rnti = Rnti::new(self.next_c_rnti.fetch_add(1, Ordering::SeqCst));
            
            // 2. Create RA procedure state
            let ra_proc = RandomAccessProcedure {
                tc_rnti,
                timing_advance: (preamble.timing_advance_us * 16.0) as u16, // Convert to TA command units
                prach_frame: detection.frame,
                prach_slot: detection.slot,
                preamble_index: preamble.preamble_index,
            };
            
            let mut ra_procs = self.ra_procedures.lock().await;
            ra_procs.push(ra_proc);
            drop(ra_procs);
            
            // 3. Schedule Random Access Response (RAR) 
            // RAR window starts at PRACH + 3 slots
            info!("Scheduled RAR for TC-RNTI {} with TA={}", tc_rnti.0, preamble.timing_advance_us);
        }
        
        Ok(())
    }
}

/// MAC subheader structure
#[derive(Debug, Clone)]
pub struct MacSubheader {
    /// Logical channel ID
    pub lcid: u8,
    /// Length field
    pub length: Option<u16>,
}

/// MAC Service Data Unit (SDU)
#[derive(Debug)]
pub struct MacSdu {
    /// Subheader
    pub subheader: MacSubheader,
    /// Payload data
    pub data: Bytes,
}

#[async_trait]
impl RrcMacInterface for EnhancedMacLayer {
    async fn send_rrc_message(&self, rnti: Rnti, msg_type: RrcMessageType, data: Bytes) -> Result<(), LayerError> {
        if !self.initialized {
            return Err(LayerError::NotInitialized);
        }
        
        info!("MAC: Sending RRC message type {:?} to RNTI {}, size: {} bytes", 
              msg_type, rnti.0, data.len());
        
        // TODO: Schedule transmission in next available slot
        // For now, just log
        match msg_type {
            RrcMessageType::RrcSetup => {
                info!("Scheduling RRC Setup (Msg4) for RNTI {}", rnti.0);
            }
            _ => {
                debug!("Scheduling RRC message type {:?}", msg_type);
            }
        }
        
        Ok(())
    }
    
    async fn allocate_c_rnti(&self) -> Result<Rnti, LayerError> {
        let rnti_value = self.next_c_rnti.fetch_add(1, Ordering::SeqCst);
        Ok(Rnti::new(rnti_value))
    }
    
    async fn schedule_rar(&self, tc_rnti: Rnti, grant: RarGrant) -> Result<(), LayerError> {
        if !self.initialized {
            return Err(LayerError::NotInitialized);
        }
        
        info!("MAC: Scheduling RAR for TC-RNTI {}, TA={}", tc_rnti.0, grant.timing_advance);
        
        // Generate RAR PDU
        let rar_pdu = self.generate_rar(tc_rnti, grant.timing_advance);
        
        // TODO: Actually schedule in next slot
        // For now, just log
        info!("Generated RAR PDU: {} bytes", rar_pdu.len());
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_enhanced_mac_initialization() {
        let config = MacConfig {
            cell_id: CellId(1),
            scs: SubcarrierSpacing::Scs15,
            bandwidth: Bandwidth::Bw10,
            max_ues: 32,
            sib1_config: default_sib1_config(CellId(1)),
        };
        
        let mut mac = EnhancedMacLayer::new(config).unwrap();
        assert!(mac.initialize().await.is_ok());
        
        // Test getting slot schedule
        let schedule = mac.get_slot_schedule(0, 0).await.unwrap();
        assert!(schedule.ssb_info.is_some());  // SSB in frame 0, slot 0
        
        // Test getting SIB1 payload
        let sib1 = mac.get_sib1_payload().await.unwrap();
        assert!(sib1.len() >= 100);
        
        // Test C-RNTI allocation
        let rnti1 = mac.allocate_c_rnti().await.unwrap();
        let rnti2 = mac.allocate_c_rnti().await.unwrap();
        assert_ne!(rnti1.0, rnti2.0);
    }
}