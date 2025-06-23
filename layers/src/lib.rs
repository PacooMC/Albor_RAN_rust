//! Protocol Stack Layers Library
//! 
//! This crate implements the 5G protocol stack layers according to 3GPP Release 16.

pub mod phy;
pub mod mac;
pub mod rlc;
pub mod pdcp;
pub mod rrc;
pub mod ngap;

use async_trait::async_trait;
use bytes::Bytes;
use thiserror::Error;

/// Common errors for protocol layers
#[derive(Error, Debug)]
pub enum LayerError {
    #[error("Invalid protocol data unit")]
    InvalidPdu,
    
    #[error("Layer not initialized")]
    NotInitialized,
    
    #[error("Resource unavailable")]
    ResourceUnavailable,
    
    #[error("Configuration error: {0}")]
    ConfigurationError(String),
    
    #[error("Processing error: {0}")]
    ProcessingError(String),
    
    #[error("Invalid configuration: {0}")]
    InvalidConfiguration(String),
    
    #[error("Initialization failed: {0}")]
    InitializationFailed(String),
    
    #[error("CRC check failed")]
    CrcFailed,
    
    #[error("Invalid state: {0}")]
    InvalidState(String),
}

/// Common trait for all protocol layers
#[async_trait]
pub trait ProtocolLayer: Send + Sync {
    /// Initialize the layer
    async fn initialize(&mut self) -> Result<(), LayerError>;
    
    /// Process incoming data from lower layer
    async fn process_uplink(&mut self, data: Bytes) -> Result<Bytes, LayerError>;
    
    /// Process outgoing data from upper layer
    async fn process_downlink(&mut self, data: Bytes) -> Result<Bytes, LayerError>;
    
    /// Shutdown the layer
    async fn shutdown(&mut self) -> Result<(), LayerError>;
}