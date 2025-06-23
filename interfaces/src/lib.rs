//! ZMQ Communication Interfaces Library
//! 
//! This crate provides ZeroMQ-based interfaces for communication with the reference UE.

pub mod zmq_handler;
pub mod message_types;
pub mod zmq_rf;

use thiserror::Error;

/// Interface errors
#[derive(Error, Debug)]
pub enum InterfaceError {
    #[error("ZMQ error: {0}")]
    ZmqError(#[from] zmq::Error),
    
    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),
    
    #[error("Invalid message format")]
    InvalidMessage,
    
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),
    
    #[error("Interface not initialized")]
    NotInitialized,
    
    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),
    
    #[error("Initialization failed: {0}")]
    InitializationFailed(String),
    
    #[error("Buffer full")]
    BufferFull,
}