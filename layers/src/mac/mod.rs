//! Medium Access Control (MAC) Layer Implementation
//! 
//! Implements the 5G NR MAC layer according to 3GPP TS 38.321

pub mod scheduler;
pub mod sib1;

use crate::{LayerError, ProtocolLayer};
use async_trait::async_trait;
use bytes::Bytes;
use tracing::{debug, info, error};
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};

pub use scheduler::{MacScheduler, SlotSchedule, SsbScheduleInfo, Sib1ScheduleInfo};
pub use sib1::{Sib1Generator, Sib1Config, default_sib1_config};
use common::types::{CellId, SubcarrierSpacing, Bandwidth};

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
}

/// Enhanced MAC layer implementation
pub struct EnhancedMacLayer {
    config: MacConfig,
    scheduler: Arc<Mutex<MacScheduler>>,
    sib1_generator: Arc<Sib1Generator>,
    sib1_payload: Arc<RwLock<Option<Bytes>>>,
    initialized: bool,
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
        })
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
        
        // TODO: Implement MAC uplink processing
        // - Demultiplex MAC PDUs
        // - Process MAC control elements (BSR, PHR, etc.)
        // - Handle Random Access procedures
        // - Forward to appropriate logical channels
        
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

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_enhanced_mac_initialization() {
        let config = MacConfig {
            cell_id: CellId(1),
            scs: SubcarrierSpacing::Scs15,
            bandwidth: Bandwidth::Bw20,
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
    }
}