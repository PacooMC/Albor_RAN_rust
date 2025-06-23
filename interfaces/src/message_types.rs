//! Message Types for GNB-UE Communication
//! 
//! Defines the message formats used for ZMQ communication

use serde::{Deserialize, Serialize};

/// Message types exchanged between GNB and UE
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum MessageType {
    /// RRC connection request
    RrcConnectionRequest,
    /// RRC connection setup
    RrcConnectionSetup,
    /// RRC connection setup complete
    RrcConnectionSetupComplete,
    /// NAS transport
    NasTransport,
    /// System information
    SystemInformation,
    /// Measurement report
    MeasurementReport,
    /// Handover command
    HandoverCommand,
    /// Data transmission
    DataTransmission,
    /// Keep alive
    KeepAlive,
}

/// Message from UE to GNB
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UeMessage {
    /// Message type
    pub msg_type: MessageType,
    /// UE identifier
    pub ue_id: u32,
    /// Cell ID
    pub cell_id: u16,
    /// Message payload
    pub payload: Vec<u8>,
    /// Timestamp (milliseconds since epoch)
    pub timestamp: u64,
}

/// Message from GNB to UE
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GnbMessage {
    /// Message type
    pub msg_type: MessageType,
    /// Cell ID
    pub cell_id: u16,
    /// Message payload
    pub payload: Vec<u8>,
    /// Timestamp (milliseconds since epoch)
    pub timestamp: u64,
}

/// RRC connection request payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RrcConnectionRequestPayload {
    /// UE identity (IMSI or TMSI)
    pub ue_identity: Vec<u8>,
    /// Establishment cause
    pub establishment_cause: u8,
}

/// RRC connection setup payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RrcConnectionSetupPayload {
    /// Radio resource configuration
    pub radio_resource_config: Vec<u8>,
    /// SRB configuration
    pub srb_config: Vec<u8>,
}

/// NAS transport payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NasTransportPayload {
    /// NAS message
    pub nas_message: Vec<u8>,
    /// Security header type
    pub security_header_type: u8,
}

/// System information payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemInformationPayload {
    /// SIB type
    pub sib_type: u8,
    /// SIB content
    pub sib_content: Vec<u8>,
}

impl UeMessage {
    /// Create a new UE message
    pub fn new(msg_type: MessageType, ue_id: u32, cell_id: u16, payload: Vec<u8>) -> Self {
        Self {
            msg_type,
            ue_id,
            cell_id,
            payload,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
        }
    }
}

impl GnbMessage {
    /// Create a new GNB message
    pub fn new(msg_type: MessageType, cell_id: u16, payload: Vec<u8>) -> Self {
        Self {
            msg_type,
            cell_id,
            payload,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_message_serialization() {
        let ue_msg = UeMessage::new(
            MessageType::RrcConnectionRequest,
            1001,
            1,
            vec![0x01, 0x02, 0x03],
        );
        
        // Serialize
        let serialized = serde_json::to_string(&ue_msg).unwrap();
        
        // Deserialize
        let deserialized: UeMessage = serde_json::from_str(&serialized).unwrap();
        
        assert_eq!(ue_msg.msg_type, deserialized.msg_type);
        assert_eq!(ue_msg.ue_id, deserialized.ue_id);
        assert_eq!(ue_msg.payload, deserialized.payload);
    }
}