//! Radio Resource Control (RRC) Layer Implementation
//! 
//! Implements the 5G NR RRC layer according to 3GPP TS 38.331

use crate::{LayerError, ProtocolLayer};
use async_trait::async_trait;
use bytes::{Bytes, BytesMut, BufMut};
use tracing::{debug, info, warn, error};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, mpsc};
use common::types::{Rnti, CellId};

/// RRC states for UE
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RrcState {
    /// RRC Idle state
    Idle,
    /// RRC Inactive state
    Inactive,
    /// RRC Connected state
    Connected,
}

/// RRC message types
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RrcMessageType {
    /// MIB (Master Information Block)
    Mib,
    /// SIB1 (System Information Block 1)
    Sib1,
    /// RRC Setup Request
    RrcSetupRequest,
    /// RRC Setup
    RrcSetup,
    /// RRC Setup Complete
    RrcSetupComplete,
    /// RRC Reconfiguration
    RrcReconfiguration,
    /// RRC Reconfiguration Complete
    RrcReconfigurationComplete,
    /// Security Mode Command
    SecurityModeCommand,
    /// Security Mode Complete
    SecurityModeComplete,
    /// UE Capability Enquiry
    UeCapabilityEnquiry,
    /// UE Capability Information
    UeCapabilityInformation,
}

/// Random Access Response Grant
#[derive(Debug, Clone)]
pub struct RarGrant {
    /// Timing advance
    pub timing_advance: u16,
    /// Uplink grant
    pub ul_grant: u32,
    /// Temporary C-RNTI
    pub tc_rnti: Rnti,
}

/// RRC Setup Request message
#[derive(Debug, Clone)]
pub struct RrcSetupRequest {
    /// UE identity (S-TMSI or random value)
    pub ue_identity: Vec<u8>,
    /// Establishment cause
    pub establishment_cause: EstablishmentCause,
}

/// RRC establishment cause
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EstablishmentCause {
    Emergency,
    HighPriorityAccess,
    MtAccess,
    MoSignalling,
    MoData,
    MoVoiceCall,
    MoVideoCall,
    MoSms,
    MpsService,
    McsService,
}

/// RRC Setup message
#[derive(Debug, Clone)]
pub struct RrcSetup {
    /// Radio bearer configuration
    pub radio_bearer_config: RadioBearerConfig,
    /// Master cell group configuration
    pub master_cell_group: Vec<u8>,
}

/// Radio Bearer Configuration
#[derive(Debug, Clone)]
pub struct RadioBearerConfig {
    /// SRB1 configuration
    pub srb1_config: Vec<u8>,
    /// Security algorithm configuration
    pub security_algorithm_config: Option<Vec<u8>>,
}

/// RRC-MAC interface for message exchange
#[async_trait]
pub trait RrcMacInterface: Send + Sync {
    /// Send RRC message to MAC for transmission
    async fn send_rrc_message(&self, rnti: Rnti, msg_type: RrcMessageType, data: Bytes) -> Result<(), LayerError>;
    
    /// Allocate C-RNTI for a UE
    async fn allocate_c_rnti(&self) -> Result<Rnti, LayerError>;
    
    /// Schedule Random Access Response
    async fn schedule_rar(&self, tc_rnti: Rnti, grant: RarGrant) -> Result<(), LayerError>;
}

/// UE context
#[derive(Debug)]
pub struct UeContext {
    /// UE identifier
    pub ue_id: u32,
    /// C-RNTI
    pub c_rnti: Rnti,
    /// Current RRC state
    pub state: RrcState,
    /// Security capabilities
    pub security_capabilities: Vec<u8>,
    /// UE identity from RRC Setup Request
    pub ue_identity: Vec<u8>,
    /// Establishment cause
    pub establishment_cause: Option<EstablishmentCause>,
}

/// RRC layer configuration
pub struct RrcConfig {
    /// System Information Block periodicity in ms
    pub sib_periodicity: u32,
    /// Maximum number of UE contexts
    pub max_ue_contexts: u16,
    /// Cell ID
    pub cell_id: CellId,
    /// PLMN ID (encoded)
    pub plmn_id: [u8; 3],
    /// Tracking Area Code
    pub tac: u32,
}

/// RRC layer implementation
pub struct RrcLayer {
    config: RrcConfig,
    initialized: bool,
    /// UE contexts indexed by C-RNTI
    ue_contexts: Arc<Mutex<HashMap<u16, UeContext>>>,
    /// MAC interface for message transmission
    mac_interface: Option<Arc<dyn RrcMacInterface>>,
    /// Next UE ID to allocate
    next_ue_id: Arc<Mutex<u32>>,
    /// Message receiver from MAC
    mac_rx: Option<mpsc::Receiver<(Rnti, Bytes)>>,
    /// Message sender to MAC
    mac_tx: Option<mpsc::Sender<(Rnti, RrcMessageType, Bytes)>>,
}

impl RrcLayer {
    /// Create a new RRC layer instance
    pub fn new(config: RrcConfig) -> Self {
        Self {
            config,
            initialized: false,
            ue_contexts: Arc::new(Mutex::new(HashMap::new())),
            mac_interface: None,
            next_ue_id: Arc::new(Mutex::new(1000)),
            mac_rx: None,
            mac_tx: None,
        }
    }
    
    /// Set MAC interface
    pub fn set_mac_interface(&mut self, mac_interface: Arc<dyn RrcMacInterface>) {
        self.mac_interface = Some(mac_interface);
    }
    
    /// Create message channels
    pub fn create_channels(&mut self) -> (mpsc::Sender<(Rnti, Bytes)>, mpsc::Receiver<(Rnti, RrcMessageType, Bytes)>) {
        let (mac_to_rrc_tx, mac_to_rrc_rx) = mpsc::channel(100);
        let (rrc_to_mac_tx, rrc_to_mac_rx) = mpsc::channel(100);
        
        self.mac_rx = Some(mac_to_rrc_rx);
        self.mac_tx = Some(rrc_to_mac_tx);
        
        (mac_to_rrc_tx, rrc_to_mac_rx)
    }
    
    /// Handle RRC Setup Request from UE
    async fn handle_rrc_setup_request(&mut self, rnti: Rnti, request: RrcSetupRequest) -> Result<(), LayerError> {
        info!("Handling RRC Setup Request from RNTI {}: cause={:?}", rnti.0, request.establishment_cause);
        
        // Allocate UE ID
        let mut ue_id_guard = self.next_ue_id.lock().await;
        let ue_id = *ue_id_guard;
        *ue_id_guard += 1;
        drop(ue_id_guard);
        
        // Create new UE context
        let ue_context = UeContext {
            ue_id,
            c_rnti: rnti,
            state: RrcState::Connected,
            security_capabilities: Vec::new(),
            ue_identity: request.ue_identity.clone(),
            establishment_cause: Some(request.establishment_cause),
        };
        
        // Store UE context
        let mut contexts = self.ue_contexts.lock().await;
        contexts.insert(rnti.0, ue_context);
        drop(contexts);
        
        // Generate RRC Setup message
        let rrc_setup = self.generate_rrc_setup(rnti).await?;
        
        // Send to MAC for transmission
        if let Some(mac_interface) = &self.mac_interface {
            mac_interface.send_rrc_message(rnti, RrcMessageType::RrcSetup, rrc_setup).await?;
            info!("Sent RRC Setup to RNTI {}", rnti.0);
        } else {
            error!("No MAC interface configured");
            return Err(LayerError::ConfigurationError("No MAC interface".into()));
        }
        
        Ok(())
    }
    
    /// Generate RRC Setup message
    async fn generate_rrc_setup(&self, rnti: Rnti) -> Result<Bytes, LayerError> {
        debug!("Generating RRC Setup for RNTI {}", rnti.0);
        
        // Create a simplified RRC Setup message
        // In a real implementation, this would use ASN.1 encoding
        let mut buf = BytesMut::new();
        
        // Message type indicator
        buf.put_u8(0x00); // RRC Setup
        
        // Transaction ID
        buf.put_u8(0x00);
        
        // Radio Bearer Config
        // SRB1 configuration
        buf.put_u8(0x01); // SRB ID
        buf.put_u8(0x00); // RLC mode (AM)
        buf.put_u8(0x05); // SN field length
        
        // Master Cell Group config (simplified)
        buf.put_u8(0x10); // Cell group ID
        buf.put_u16(self.config.cell_id.0); // Cell ID
        
        // Physical cell configuration
        buf.put_u8(0x20); // Config type
        buf.put_u8(0x00); // Default config
        
        Ok(buf.freeze())
    }
    
    /// Handle RRC Setup Complete from UE
    async fn handle_rrc_setup_complete(&mut self, rnti: Rnti, _data: Bytes) -> Result<(), LayerError> {
        info!("Handling RRC Setup Complete from RNTI {}", rnti.0);
        
        let mut contexts = self.ue_contexts.lock().await;
        if let Some(ue_context) = contexts.get_mut(&rnti.0) {
            ue_context.state = RrcState::Connected;
            info!("UE {} (RNTI {}) is now RRC Connected", ue_context.ue_id, rnti.0);
            
            // TODO: Notify upper layers (NGAP) about new UE connection
            // This would trigger Initial UE Message to AMF
            
            Ok(())
        } else {
            warn!("No UE context found for RNTI {}", rnti.0);
            Err(LayerError::InvalidState("No UE context".into()))
        }
    }
    
    /// Parse RRC message type from data
    fn parse_message_type(&self, data: &[u8]) -> Option<RrcMessageType> {
        if data.is_empty() {
            return None;
        }
        
        // Simple message type detection (would be ASN.1 in real implementation)
        match data[0] & 0x3F {
            0x00 => Some(RrcMessageType::RrcSetupRequest),
            0x01 => Some(RrcMessageType::RrcSetup),
            0x02 => Some(RrcMessageType::RrcSetupComplete),
            0x10 => Some(RrcMessageType::SecurityModeCommand),
            0x11 => Some(RrcMessageType::SecurityModeComplete),
            0x20 => Some(RrcMessageType::RrcReconfiguration),
            0x21 => Some(RrcMessageType::RrcReconfigurationComplete),
            _ => None,
        }
    }
    
    /// Parse RRC Setup Request
    fn parse_rrc_setup_request(&self, data: &[u8]) -> Result<RrcSetupRequest, LayerError> {
        if data.len() < 3 {
            return Err(LayerError::InvalidPdu);
        }
        
        // Skip message type
        let mut idx = 1;
        
        // Parse UE identity length
        let id_len = data[idx] as usize;
        idx += 1;
        
        if data.len() < idx + id_len + 1 {
            return Err(LayerError::InvalidPdu);
        }
        
        // Extract UE identity
        let ue_identity = data[idx..idx + id_len].to_vec();
        idx += id_len;
        
        // Parse establishment cause
        let cause_byte = data[idx];
        let establishment_cause = match cause_byte {
            0 => EstablishmentCause::Emergency,
            1 => EstablishmentCause::HighPriorityAccess,
            2 => EstablishmentCause::MtAccess,
            3 => EstablishmentCause::MoSignalling,
            4 => EstablishmentCause::MoData,
            5 => EstablishmentCause::MoVoiceCall,
            6 => EstablishmentCause::MoVideoCall,
            7 => EstablishmentCause::MoSms,
            _ => EstablishmentCause::MoData, // Default
        };
        
        Ok(RrcSetupRequest {
            ue_identity,
            establishment_cause,
        })
    }
}

#[async_trait]
impl ProtocolLayer for RrcLayer {
    async fn initialize(&mut self) -> Result<(), LayerError> {
        info!("Initializing RRC layer");
        debug!("RRC config: sib_periodicity={}ms, max_ue_contexts={}", 
               self.config.sib_periodicity,
               self.config.max_ue_contexts);
        
        // TODO: Initialize RRC resources
        // - Setup system information broadcasting
        // - Initialize measurement configuration
        // - Setup security contexts
        
        self.initialized = true;
        info!("RRC layer initialized successfully");
        Ok(())
    }
    
    async fn process_uplink(&mut self, data: Bytes) -> Result<Bytes, LayerError> {
        if !self.initialized {
            return Err(LayerError::NotInitialized);
        }
        
        debug!("RRC processing uplink data: {} bytes", data.len());
        
        // In a real system, we'd get RNTI from lower layers
        // For now, assume it's embedded in the message or use a default
        let rnti = Rnti::new(0x4601); // Temporary, should come from MAC
        
        // Parse message type
        if let Some(msg_type) = self.parse_message_type(&data) {
            info!("Received RRC message type: {:?}", msg_type);
            
            match msg_type {
                RrcMessageType::RrcSetupRequest => {
                    match self.parse_rrc_setup_request(&data) {
                        Ok(request) => {
                            if let Err(e) = self.handle_rrc_setup_request(rnti, request).await {
                                error!("Failed to handle RRC Setup Request: {}", e);
                            }
                        }
                        Err(e) => {
                            error!("Failed to parse RRC Setup Request: {}", e);
                        }
                    }
                }
                RrcMessageType::RrcSetupComplete => {
                    if let Err(e) = self.handle_rrc_setup_complete(rnti, data.clone()).await {
                        error!("Failed to handle RRC Setup Complete: {}", e);
                    }
                }
                _ => {
                    debug!("Unhandled RRC message type: {:?}", msg_type);
                }
            }
        } else {
            warn!("Unknown RRC message type");
        }
        
        // Pass through to upper layers if needed
        Ok(data)
    }
    
    async fn process_downlink(&mut self, data: Bytes) -> Result<Bytes, LayerError> {
        if !self.initialized {
            return Err(LayerError::NotInitialized);
        }
        
        debug!("RRC processing downlink data: {} bytes", data.len());
        
        // TODO: Encode RRC messages for transmission
        
        Ok(data)
    }
    
    async fn shutdown(&mut self) -> Result<(), LayerError> {
        info!("Shutting down RRC layer");
        
        // Release all UE contexts
        let mut contexts = self.ue_contexts.lock().await;
        let ue_count = contexts.len();
        if ue_count > 0 {
            warn!("Releasing {} UE contexts", ue_count);
            contexts.clear();
        }
        drop(contexts);
        
        self.initialized = false;
        Ok(())
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use common::types::CellId;
    
    #[tokio::test]
    async fn test_rrc_initialization() {
        let config = RrcConfig {
            sib_periodicity: 160,
            max_ue_contexts: 100,
            cell_id: CellId(1),
            plmn_id: [0x00, 0xF1, 0x10], // 00101
            tac: 7,
        };
        
        let mut rrc = RrcLayer::new(config);
        assert!(rrc.initialize().await.is_ok());
    }
    
    #[tokio::test]
    async fn test_ue_context_creation() {
        let config = RrcConfig {
            sib_periodicity: 160,
            max_ue_contexts: 100,
            cell_id: CellId(1),
            plmn_id: [0x00, 0xF1, 0x10], // 00101
            tac: 7,
        };
        
        let mut rrc = RrcLayer::new(config);
        rrc.initialize().await.unwrap();
        
        // Test UE context creation
        let request = RrcSetupRequest {
            ue_identity: vec![0x01, 0x02, 0x03, 0x04],
            establishment_cause: EstablishmentCause::MoData,
        };
        let rnti = Rnti::new(0x4601);
        assert!(rrc.handle_rrc_setup_request(rnti, request).await.is_ok());
        
        let contexts = rrc.ue_contexts.lock().await;
        assert_eq!(contexts.len(), 1);
        assert_eq!(contexts[&rnti.0].state, RrcState::Connected);
    }
}