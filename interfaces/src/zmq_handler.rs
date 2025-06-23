//! ZeroMQ Handler Implementation
//! 
//! Manages ZeroMQ sockets for communication with reference UE

use crate::{InterfaceError, message_types::{UeMessage, GnbMessage}};
use tracing::{debug, info};
use zmq::Context;
use std::sync::Arc;
use tokio::sync::Mutex;

/// ZMQ socket types
#[derive(Debug, Clone, Copy)]
pub enum SocketType {
    /// Request-Reply pattern (GNB as server)
    Rep,
    /// Publish-Subscribe pattern (GNB as publisher)
    Pub,
    /// Push-Pull pattern (GNB as pusher)
    Push,
}

/// ZMQ handler configuration
pub struct ZmqConfig {
    /// Socket type
    pub socket_type: SocketType,
    /// Bind address for incoming connections
    pub bind_address: String,
    /// High water mark for outgoing messages
    pub hwm: i32,
}

/// ZMQ handler for UE communication
pub struct ZmqHandler {
    config: ZmqConfig,
    context: Context,
    socket: Option<zmq::Socket>,
    initialized: bool,
}

impl ZmqHandler {
    /// Create a new ZMQ handler
    pub fn new(config: ZmqConfig) -> Result<Self, InterfaceError> {
        let context = Context::new();
        
        Ok(Self {
            config,
            context,
            socket: None,
            initialized: false,
        })
    }
    
    /// Initialize the ZMQ handler
    pub fn initialize(&mut self) -> Result<(), InterfaceError> {
        info!("Initializing ZMQ handler");
        
        let socket = match self.config.socket_type {
            SocketType::Rep => {
                let sock = self.context.socket(zmq::REP)?;
                sock.bind(&self.config.bind_address)?;
                info!("ZMQ REP socket bound to {}", self.config.bind_address);
                sock
            }
            SocketType::Pub => {
                let sock = self.context.socket(zmq::PUB)?;
                sock.bind(&self.config.bind_address)?;
                info!("ZMQ PUB socket bound to {}", self.config.bind_address);
                sock
            }
            SocketType::Push => {
                let sock = self.context.socket(zmq::PUSH)?;
                sock.bind(&self.config.bind_address)?;
                info!("ZMQ PUSH socket bound to {}", self.config.bind_address);
                sock
            }
        };
        
        socket.set_sndhwm(self.config.hwm)?;
        socket.set_rcvhwm(self.config.hwm)?;
        
        self.socket = Some(socket);
        self.initialized = true;
        
        info!("ZMQ handler initialized successfully");
        Ok(())
    }
    
    /// Receive message from UE
    pub fn receive(&self) -> Result<UeMessage, InterfaceError> {
        if !self.initialized {
            return Err(InterfaceError::NotInitialized);
        }
        
        let socket = self.socket.as_ref().unwrap();
        
        // Receive raw bytes
        let msg = socket.recv_bytes(0)?;
        debug!("Received {} bytes from UE", msg.len());
        
        // Deserialize message
        let ue_msg: UeMessage = serde_json::from_slice(&msg)?;
        debug!("Received UE message: {:?}", ue_msg.msg_type);
        
        Ok(ue_msg)
    }
    
    /// Send message to UE
    pub fn send(&self, message: &GnbMessage) -> Result<(), InterfaceError> {
        if !self.initialized {
            return Err(InterfaceError::NotInitialized);
        }
        
        let socket = self.socket.as_ref().unwrap();
        
        // Serialize message
        let data = serde_json::to_vec(message)?;
        debug!("Sending {} bytes to UE", data.len());
        
        // Send bytes
        socket.send(&data, 0)?;
        debug!("Sent GNB message: {:?}", message.msg_type);
        
        Ok(())
    }
    
    /// Shutdown the handler
    pub fn shutdown(&mut self) -> Result<(), InterfaceError> {
        info!("Shutting down ZMQ handler");
        
        if let Some(socket) = self.socket.take() {
            drop(socket);
        }
        
        self.initialized = false;
        info!("ZMQ handler shut down");
        Ok(())
    }
}

/// Async wrapper for ZMQ handler
pub struct AsyncZmqHandler {
    inner: Arc<Mutex<ZmqHandler>>,
}

impl AsyncZmqHandler {
    /// Create new async ZMQ handler
    pub fn new(config: ZmqConfig) -> Result<Self, InterfaceError> {
        let handler = ZmqHandler::new(config)?;
        Ok(Self {
            inner: Arc::new(Mutex::new(handler)),
        })
    }
    
    /// Initialize the handler
    pub async fn initialize(&self) -> Result<(), InterfaceError> {
        let mut handler = self.inner.lock().await;
        handler.initialize()
    }
    
    /// Receive message asynchronously
    pub async fn receive(&self) -> Result<UeMessage, InterfaceError> {
        // TODO: Implement proper async receiving using tokio tasks
        let handler = self.inner.lock().await;
        handler.receive()
    }
    
    /// Send message asynchronously
    pub async fn send(&self, message: &GnbMessage) -> Result<(), InterfaceError> {
        let handler = self.inner.lock().await;
        handler.send(message)
    }
    
    /// Shutdown the handler
    pub async fn shutdown(&self) -> Result<(), InterfaceError> {
        let mut handler = self.inner.lock().await;
        handler.shutdown()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::message_types::MessageType;
    
    #[test]
    fn test_zmq_handler_creation() {
        let config = ZmqConfig {
            socket_type: SocketType::Rep,
            bind_address: "tcp://127.0.0.1:5555".to_string(),
            hwm: 1000,
        };
        
        let handler = ZmqHandler::new(config);
        assert!(handler.is_ok());
    }
}