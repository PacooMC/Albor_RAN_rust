//! ZMQ RF Driver for IQ Sample Exchange
//! 
//! This module implements the ZMQ-based RF driver for exchanging IQ samples
//! with srsUE and other compatible software radios.

use crate::InterfaceError;
use num_complex::Complex32;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, error, info, warn, trace};
use zmq::{Context, Socket};
use std::time::{Duration, Instant};

/// Default sample rate for LTE/5G NR (30.72 MHz)
pub const DEFAULT_SAMPLE_RATE: f64 = 30.72e6;

/// Default ZMQ ports
pub const DEFAULT_TX_PORT: u16 = 2000;
pub const DEFAULT_RX_PORT: u16 = 2001;

/// Default base sample rate (for 100 PRB cell)
pub const DEFAULT_BASE_SRATE: f64 = 23.04e6;

/// ZMQ RF configuration
#[derive(Debug, Clone)]
pub struct ZmqRfConfig {
    /// Sample rate in Hz
    pub sample_rate: f64,
    /// Number of channels (MIMO)
    pub num_channels: usize,
    /// TX binding address
    pub tx_address: String,
    /// RX connection address
    pub rx_address: String,
    /// Buffer size for samples
    pub buffer_size: usize,
    /// Ring buffer size (number of buffers)
    pub ring_buffer_size: usize,
    /// Transmit gain in dB
    pub tx_gain: f32,
    /// Receive gain in dB
    pub rx_gain: f32,
}

impl Default for ZmqRfConfig {
    fn default() -> Self {
        Self {
            sample_rate: DEFAULT_SAMPLE_RATE,
            num_channels: 1,
            tx_address: format!("tcp://*:{}", DEFAULT_TX_PORT),
            rx_address: format!("tcp://localhost:{}", DEFAULT_RX_PORT),
            buffer_size: 1920, // One slot at 30.72 MHz
            ring_buffer_size: 16384, // Massive increase to match srsRAN approach (10+ subframes worth)
            tx_gain: 0.0,
            rx_gain: 0.0,
        }
    }
}

impl ZmqRfConfig {
    /// Parse device arguments in srsRAN format
    /// Format: "key1=value1,key2=value2,..."
    /// Supports indexed port names: tx_port0, rx_port0, tx_port1, rx_port1, etc.
    pub fn from_device_args(args: &str, num_channels: usize) -> Result<Self, InterfaceError> {
        let mut config = Self::default();
        config.num_channels = num_channels;
        
        // Parse key=value pairs
        let pairs: Vec<&str> = args.split(',').collect();
        
        for pair in pairs {
            let parts: Vec<&str> = pair.trim().split('=').collect();
            if parts.len() != 2 {
                continue;
            }
            
            let key = parts[0].trim();
            let value = parts[1].trim();
            
            match key {
                "base_srate" => {
                    config.sample_rate = value.parse::<f64>()
                        .map_err(|_| InterfaceError::InvalidConfig("Invalid base_srate".to_string()))?;
                }
                "tx_gain" => {
                    config.tx_gain = value.parse::<f32>()
                        .map_err(|_| InterfaceError::InvalidConfig("Invalid tx_gain".to_string()))?;
                }
                "rx_gain" => {
                    config.rx_gain = value.parse::<f32>()
                        .map_err(|_| InterfaceError::InvalidConfig("Invalid rx_gain".to_string()))?;
                }
                _ => {
                    // Check for indexed port names (tx_port0, rx_port0, etc.)
                    if key.starts_with("tx_port") {
                        // For now, use the first channel's port
                        if key == "tx_port" || key == "tx_port0" {
                            config.tx_address = value.to_string();
                        }
                    } else if key.starts_with("rx_port") {
                        // For now, use the first channel's port
                        if key == "rx_port" || key == "rx_port0" {
                            config.rx_address = value.to_string();
                        }
                    }
                }
            }
        }
        
        Ok(config)
    }
}

/// Sample buffer for IQ data
#[derive(Clone)]
pub struct IqBuffer {
    /// Complex IQ samples
    pub samples: Vec<Complex32>,
    /// Timestamp in samples
    pub timestamp: u64,
    /// Channel index
    pub channel: usize,
}

impl IqBuffer {
    /// Create a new IQ buffer
    pub fn new(size: usize, channel: usize) -> Self {
        Self {
            samples: vec![Complex32::new(0.0, 0.0); size],
            timestamp: 0,
            channel,
        }
    }
    
    /// Create from raw samples
    pub fn from_samples(samples: Vec<Complex32>, timestamp: u64, channel: usize) -> Self {
        Self {
            samples,
            timestamp,
            channel,
        }
    }
}

/// State for TX socket handling
#[derive(Debug, Clone, Copy, PartialEq)]
enum TxState {
    WaitingForRequest,
    RequestReceived,
}

/// State for RX socket handling
#[derive(Debug, Clone, Copy, PartialEq)]
enum RxState {
    ReadyToRequest,
    WaitingForResponse,
}

/// Convert IQ buffer to raw bytes (srsRAN format)
fn iq_buffer_to_bytes(buffer: &IqBuffer) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(buffer.samples.len() * 8);
    
    // srsRAN expects raw cf_t (complex float) samples
    // Each complex sample is 8 bytes (4 bytes real + 4 bytes imag)
    for sample in &buffer.samples {
        bytes.extend_from_slice(&sample.re.to_le_bytes());
        bytes.extend_from_slice(&sample.im.to_le_bytes());
    }
    
    bytes
}

/// Convert raw bytes to IQ buffer (srsRAN format)
fn bytes_to_iq_buffer(bytes: &[u8], timestamp: u64, channel: usize) -> Result<IqBuffer, InterfaceError> {
    if bytes.len() % 8 != 0 {
        return Err(InterfaceError::InvalidMessage);
    }
    
    let sample_count = bytes.len() / 8;
    let mut samples = Vec::with_capacity(sample_count);
    
    let mut offset = 0;
    for _ in 0..sample_count {
        let real = f32::from_le_bytes([
            bytes[offset], bytes[offset + 1],
            bytes[offset + 2], bytes[offset + 3],
        ]);
        let imag = f32::from_le_bytes([
            bytes[offset + 4], bytes[offset + 5],
            bytes[offset + 6], bytes[offset + 7],
        ]);
        samples.push(Complex32::new(real, imag));
        offset += 8;
    }
    
    Ok(IqBuffer::from_samples(samples, timestamp, channel))
}

/// TX/RX statistics
#[derive(Debug, Default, Clone)]
pub struct RfStats {
    pub tx_samples: u64,
    pub rx_samples: u64,
    pub tx_underruns: u64,
    pub rx_overruns: u64,
    pub tx_late_packets: u64,
    pub rx_late_packets: u64,
}

/// ZMQ RF driver
pub struct ZmqRfDriver {
    config: ZmqRfConfig,
    context: Context,
    tx_socket: Option<Socket>,
    rx_socket: Option<Socket>,
    tx_timestamp: Arc<RwLock<u64>>,
    rx_timestamp: Arc<RwLock<u64>>,
    stats: Arc<RwLock<RfStats>>,
    running: Arc<RwLock<bool>>,
    tx_state: Arc<RwLock<TxState>>,
    rx_state: Arc<RwLock<RxState>>,
    tx_buffer: Arc<RwLock<Option<IqBuffer>>>,
    // Circular buffer for continuous TX flow (like srsRAN)
    tx_circular_buffer: Arc<RwLock<Vec<IqBuffer>>>,
    tx_circular_write_idx: Arc<RwLock<usize>>,
    tx_circular_read_idx: Arc<RwLock<usize>>,
}

impl ZmqRfDriver {
    /// Create a new ZMQ RF driver
    pub fn new(config: ZmqRfConfig) -> Result<Self, InterfaceError> {
        let context = Context::new();
        
        // Initialize circular buffer (like srsRAN)
        let circular_buffer_size = config.ring_buffer_size;
        let mut tx_circular_buffer = Vec::with_capacity(circular_buffer_size);
        for _ in 0..circular_buffer_size {
            tx_circular_buffer.push(IqBuffer::new(config.buffer_size, 0));
        }
        
        Ok(Self {
            config,
            context,
            tx_socket: None,
            rx_socket: None,
            tx_timestamp: Arc::new(RwLock::new(0)),
            rx_timestamp: Arc::new(RwLock::new(0)),
            stats: Arc::new(RwLock::new(RfStats::default())),
            running: Arc::new(RwLock::new(false)),
            tx_state: Arc::new(RwLock::new(TxState::WaitingForRequest)),
            rx_state: Arc::new(RwLock::new(RxState::ReadyToRequest)),
            tx_buffer: Arc::new(RwLock::new(None)),
            tx_circular_buffer: Arc::new(RwLock::new(tx_circular_buffer)),
            tx_circular_write_idx: Arc::new(RwLock::new(0)),
            tx_circular_read_idx: Arc::new(RwLock::new(0)),
        })
    }
    
    /// Initialize the RF driver
    pub async fn initialize(&mut self) -> Result<(), InterfaceError> {
        info!("Initializing ZMQ RF driver");
        info!("Sample rate: {} MHz", self.config.sample_rate / 1e6);
        info!("Channels: {}", self.config.num_channels);
        
        // Create TX socket (REP - receives requests and sends TX samples)
        // TX uses REP socket and binds (waits for requests from UE)
        info!("Creating TX socket (REP) to bind to {}", self.config.tx_address);
        let tx_socket = self.context.socket(zmq::REP)?;
        tx_socket.bind(&self.config.tx_address)?;
        tx_socket.set_rcvtimeo(1)?; // 1ms timeout for non-blocking
        tx_socket.set_sndtimeo(100)?; // 100ms timeout for sending
        info!("TX socket bound to {} (REP mode) - waiting for UE requests", self.config.tx_address);
        
        // Create RX socket (REQ - sends requests and receives RX samples)  
        // RX uses REQ socket and connects (sends requests to UE)
        info!("Creating RX socket (REQ) to connect to {}", self.config.rx_address);
        let rx_socket = self.context.socket(zmq::REQ)?;
        rx_socket.connect(&self.config.rx_address)?;
        rx_socket.set_rcvtimeo(100)?; // 100ms timeout
        rx_socket.set_sndtimeo(100)?; // 100ms timeout
        info!("RX socket connected to {} (REQ mode) - will send requests to UE", self.config.rx_address);
        
        self.tx_socket = Some(tx_socket);
        self.rx_socket = Some(rx_socket);
        
        *self.running.write().await = true;
        
        info!("ZMQ RF driver initialized");
        Ok(())
    }
    
    /// Check for TX request and handle transmission (srsRAN-compatible)
    pub async fn handle_tx_request(&self) -> Result<(), InterfaceError> {
        if !*self.running.read().await {
            return Err(InterfaceError::NotInitialized);
        }
        
        let tx_socket = self.tx_socket.as_ref()
            .ok_or(InterfaceError::NotInitialized)?;
        
        let current_state = *self.tx_state.read().await;
        
        match current_state {
            TxState::WaitingForRequest => {
                // Try to receive dummy request byte from UE (REQ-REP pattern)
                let mut dummy_byte = [0u8; 1];
                match tx_socket.recv_into(&mut dummy_byte, zmq::DONTWAIT) {
                    Ok(_) => {
                        info!("TX: Received request from UE (dummy byte: 0x{:02X})", dummy_byte[0]);
                        *self.tx_state.write().await = TxState::RequestReceived;
                        
                        // Log timing information
                        static mut LAST_REQUEST_TIME: Option<std::time::Instant> = None;
                        let now = std::time::Instant::now();
                        unsafe {
                            if let Some(last) = LAST_REQUEST_TIME {
                                let delta = now.duration_since(last);
                                debug!("TX: Time since last request: {:?}", delta);
                            }
                            LAST_REQUEST_TIME = Some(now);
                        }
                        
                        // Log circular buffer status
                        let read_idx = *self.tx_circular_read_idx.read().await;
                        let write_idx = *self.tx_circular_write_idx.read().await;
                        let buffer_size = self.tx_circular_buffer.read().await.len();
                        let used = if write_idx >= read_idx {
                            write_idx - read_idx
                        } else {
                            buffer_size - read_idx + write_idx
                        };
                        debug!("TX: Circular buffer status: {}/{} buffers used", used, buffer_size);
                        
                        // Immediately try to send response (like srsRAN)
                        return self.send_tx_response().await;
                    }
                    Err(zmq::Error::EAGAIN) => {
                        // No request yet, this is normal in non-blocking mode
                        // Log periodically to track state
                        static mut COUNTER: u64 = 0;
                        unsafe {
                            COUNTER += 1;
                            if COUNTER % 10000 == 0 {
                                trace!("TX: Still waiting for UE request (checked {} times)", COUNTER);
                            }
                        }
                        Ok(())
                    }
                    Err(zmq::Error::EFSM) => {
                        // Finite state machine error - socket in wrong state
                        warn!("TX: ZMQ FSM error - socket in wrong state for recv");
                        Ok(())
                    }
                    Err(e) => {
                        error!("TX: Error receiving request: {}", e);
                        Err(InterfaceError::ZmqError(e))
                    }
                }
            }
            TxState::RequestReceived => {
                // We have a pending request, try to send response
                debug!("TX: Have pending request, sending response");
                self.send_tx_response().await
            }
        }
    }
    
    /// Send TX response (samples) to UE - matches srsRAN behavior
    async fn send_tx_response(&self) -> Result<(), InterfaceError> {
        let tx_socket = self.tx_socket.as_ref()
            .ok_or(InterfaceError::NotInitialized)?;
        
        // Try to get data from circular buffer (like srsRAN)
        let buffer = self.get_tx_samples_from_circular_buffer().await;
        
        if let Some(buffer) = buffer {
            // Convert to srsRAN format (raw cf_t samples)
            let bytes = iq_buffer_to_bytes(&buffer);
            
            // DEBUG: Check if we have non-zero samples
            let non_zero_count = buffer.samples.iter().filter(|s| s.norm() > 0.0).count();
            if non_zero_count > 0 {
                // Calculate signal statistics
                let avg_power: f32 = buffer.samples.iter().map(|s| s.norm_sqr()).sum::<f32>() / buffer.samples.len() as f32;
                let peak_power: f32 = buffer.samples.iter().map(|s| s.norm_sqr()).fold(0.0, f32::max);
                let avg_power_db = 10.0 * avg_power.log10();
                let peak_power_db = 10.0 * peak_power.log10();
                
                // Show first few non-zero samples
                let first_samples: Vec<String> = buffer.samples.iter()
                    .take(10)
                    .filter(|s| s.norm() > 0.0)
                    .take(5)
                    .map(|s| format!("({:.3}+{:.3}j)", s.re, s.im))
                    .collect();
                
                info!("TX: Sending {} samples ({} non-zero), timestamp={}", 
                      buffer.samples.len(), non_zero_count, buffer.timestamp);
                info!("TX: Signal power: avg={:.3} ({:.1} dB), peak={:.3} ({:.1} dB)",
                      avg_power, avg_power_db, peak_power, peak_power_db);
                if !first_samples.is_empty() {
                    info!("TX: First non-zero samples: {}", first_samples.join(", "));
                }
                info!("TX: Byte size: {} bytes", bytes.len());
            }
            
            // Send the samples
            match tx_socket.send(&bytes, 0) {
                Ok(_) => {
                    // Update timestamp
                    *self.tx_timestamp.write().await = buffer.timestamp + buffer.samples.len() as u64;
                    
                    // Update stats
                    let mut stats = self.stats.write().await;
                    stats.tx_samples += buffer.samples.len() as u64;
                    
                    // Go back to waiting for request
                    *self.tx_state.write().await = TxState::WaitingForRequest;
                    Ok(())
                }
                Err(e) => {
                    error!("Failed to send TX samples: {}", e);
                    // On error, don't consume the buffer
                    Err(InterfaceError::ZmqError(e))
                }
            }
        } else {
            // No data available, send zeros (underrun)
            let zero_buffer = IqBuffer::new(self.config.buffer_size, 0);
            let bytes = iq_buffer_to_bytes(&zero_buffer);
            
            match tx_socket.send(&bytes, 0) {
                Ok(_) => {
                    warn!("TX: Sent {} zero samples (underrun) - no data in circular buffer!", zero_buffer.samples.len());
                    let mut stats = self.stats.write().await;
                    stats.tx_underruns += 1;
                    *self.tx_state.write().await = TxState::WaitingForRequest;
                    Ok(())
                }
                Err(e) => {
                    error!("Failed to send zero samples: {}", e);
                    Err(InterfaceError::ZmqError(e))
                }
            }
        }
    }
    
    /// Get samples from circular buffer (like srsRAN)
    async fn get_tx_samples_from_circular_buffer(&self) -> Option<IqBuffer> {
        let circular_buffer = self.tx_circular_buffer.read().await;
        let read_idx = *self.tx_circular_read_idx.read().await;
        let write_idx = *self.tx_circular_write_idx.read().await;
        
        // Check if buffer is empty
        if read_idx == write_idx {
            trace!("TX circular buffer empty: read={}, write={}", read_idx, write_idx);
            return None;
        }
        
        // Get buffer at read index
        let buffer = circular_buffer[read_idx].clone();
        
        // Always advance read index to prevent stalling
        let mut read_idx_mut = self.tx_circular_read_idx.write().await;
        *read_idx_mut = (*read_idx_mut + 1) % circular_buffer.len();
        drop(read_idx_mut);
        
        // Log buffer utilization
        let used = if write_idx >= read_idx {
            write_idx - read_idx
        } else {
            circular_buffer.len() - read_idx + write_idx
        };
        trace!("TX circular buffer: used={}/{}, returning buffer at index {}", 
               used, circular_buffer.len(), read_idx);
        
        Some(buffer)
    }
    
    /// Queue samples for transmission (using circular buffer like srsRAN)
    pub async fn transmit(&self, buffer: &IqBuffer) -> Result<(), InterfaceError> {
        if !*self.running.read().await {
            return Err(InterfaceError::NotInitialized);
        }
        
        // DEBUG: Check buffer content
        let non_zero_count = buffer.samples.iter().filter(|s| s.norm() > 0.0).count();
        debug!("transmit() called with {} non-zero samples out of {}", non_zero_count, buffer.samples.len());
        
        // Add to circular buffer
        let mut circular_buffer = self.tx_circular_buffer.write().await;
        let mut write_idx = self.tx_circular_write_idx.write().await;
        let read_idx = *self.tx_circular_read_idx.read().await;
        
        // Check if buffer is full
        let next_write_idx = (*write_idx + 1) % circular_buffer.len();
        if next_write_idx == read_idx {
            // Buffer overflow - implement overwrite strategy like real RF drivers
            // Move read pointer forward to make space (drop oldest sample)
            let mut read_idx_mut = self.tx_circular_read_idx.write().await;
            *read_idx_mut = (*read_idx_mut + 1) % circular_buffer.len();
            drop(read_idx_mut); // Release lock
            
            let mut stats = self.stats.write().await;
            stats.tx_underruns += 1;
            
            // Log periodically to avoid spam
            static mut OVERFLOW_COUNT: u64 = 0;
            unsafe {
                OVERFLOW_COUNT += 1;
                if OVERFLOW_COUNT % 1000 == 1 {
                    warn!("TX circular buffer overflow #{} - dropping oldest samples", OVERFLOW_COUNT);
                }
            }
        }
        
        // Copy samples to circular buffer
        circular_buffer[*write_idx] = buffer.clone();
        *write_idx = next_write_idx;
        
        Ok(())
    }
    
    /// Receive IQ samples
    pub async fn receive(&self, channel: usize) -> Result<Option<IqBuffer>, InterfaceError> {
        if !*self.running.read().await {
            return Err(InterfaceError::NotInitialized);
        }
        
        let rx_socket = self.rx_socket.as_ref()
            .ok_or(InterfaceError::NotInitialized)?;
        
        // Check current state
        let current_state = *self.rx_state.read().await;
        
        match current_state {
            RxState::ReadyToRequest => {
                // Try to send request
                let dummy: &[u8] = &[0];  // Use 0 like srsRAN
                match rx_socket.send(dummy, zmq::DONTWAIT) {
                    Ok(_) => {
                        debug!("Sent RX request (dummy byte: 0x00)");
                        // Update state to waiting for response
                        *self.rx_state.write().await = RxState::WaitingForResponse;
                        
                        // Try to receive immediately (might be ready)
                        match rx_socket.recv_bytes(zmq::DONTWAIT) {
                            Ok(bytes) => {
                                // Got response immediately
                                let timestamp = *self.rx_timestamp.read().await;
                                let buffer = bytes_to_iq_buffer(&bytes, timestamp, channel)?;
                                
                                debug!("Received {} samples", buffer.samples.len());
                                
                                // Update timestamp
                                *self.rx_timestamp.write().await = timestamp + buffer.samples.len() as u64;
                                
                                // Update stats
                                let mut stats = self.stats.write().await;
                                stats.rx_samples += buffer.samples.len() as u64;
                                
                                // Reset state for next request
                                *self.rx_state.write().await = RxState::ReadyToRequest;
                                
                                Ok(Some(buffer))
                            }
                            Err(zmq::Error::EAGAIN) => {
                                // No data yet, stay in WaitingForResponse state
                                Ok(None)
                            }
                            Err(e) => {
                                error!("Failed to receive RX samples: {}", e);
                                // Reset state on error
                                *self.rx_state.write().await = RxState::ReadyToRequest;
                                Err(InterfaceError::ZmqError(e))
                            }
                        }
                    }
                    Err(zmq::Error::EAGAIN) => {
                        // This shouldn't happen in ReadyToRequest state
                        // But if it does, try to recover by receiving
                        debug!("REQ socket in unexpected state, attempting recovery");
                        match rx_socket.recv_bytes(zmq::DONTWAIT) {
                            Ok(bytes) => {
                                // Found pending data, process it
                                let timestamp = *self.rx_timestamp.read().await;
                                let buffer = bytes_to_iq_buffer(&bytes, timestamp, channel)?;
                                *self.rx_timestamp.write().await = timestamp + buffer.samples.len() as u64;
                                let mut stats = self.stats.write().await;
                                stats.rx_samples += buffer.samples.len() as u64;
                                // Reset state
                                *self.rx_state.write().await = RxState::ReadyToRequest;
                                return Ok(Some(buffer));
                            }
                            Err(_) => {
                                // No pending data, return None
                                return Ok(None);
                            }
                        }
                    }
                    Err(e) => {
                        error!("Failed to send RX request: {}", e);
                        return Err(InterfaceError::ZmqError(e));
                    }
                }
            }
            RxState::WaitingForResponse => {
                // Try to receive response
                match rx_socket.recv_bytes(zmq::DONTWAIT) {
                    Ok(bytes) => {
                        // Parse raw cf_t samples
                        let timestamp = *self.rx_timestamp.read().await;
                        let buffer = bytes_to_iq_buffer(&bytes, timestamp, channel)?;
                        
                        debug!("Received {} samples", buffer.samples.len());
                        
                        // Update timestamp
                        *self.rx_timestamp.write().await = timestamp + buffer.samples.len() as u64;
                        
                        // Update stats
                        let mut stats = self.stats.write().await;
                        stats.rx_samples += buffer.samples.len() as u64;
                        
                        // Reset state for next request
                        *self.rx_state.write().await = RxState::ReadyToRequest;
                        
                        Ok(Some(buffer))
                    }
                    Err(zmq::Error::EAGAIN) => {
                        // Still waiting for response
                        Ok(None)
                    }
                    Err(e) => {
                        error!("Failed to receive RX samples: {}", e);
                        // Reset state on error
                        *self.rx_state.write().await = RxState::ReadyToRequest;
                        Err(InterfaceError::ZmqError(e))
                    }
                }
            }
        }
    }
    
    /// Get current TX timestamp
    pub async fn tx_timestamp(&self) -> u64 {
        *self.tx_timestamp.read().await
    }
    
    /// Get current RX timestamp
    pub async fn rx_timestamp(&self) -> u64 {
        *self.rx_timestamp.read().await
    }
    
    /// Get RF statistics
    pub async fn stats(&self) -> RfStats {
        (*self.stats.read().await).clone()
    }
    
    /// Shutdown the RF driver
    pub async fn shutdown(&mut self) -> Result<(), InterfaceError> {
        info!("Shutting down ZMQ RF driver");
        
        *self.running.write().await = false;
        
        // Close sockets
        if let Some(socket) = self.tx_socket.take() {
            drop(socket);
        }
        if let Some(socket) = self.rx_socket.take() {
            drop(socket);
        }
        
        // Print final stats
        let stats = self.stats.read().await;
        info!("Final RF stats:");
        info!("  TX samples: {}", stats.tx_samples);
        info!("  RX samples: {}", stats.rx_samples);
        info!("  TX underruns: {}", stats.tx_underruns);
        info!("  RX overruns: {}", stats.rx_overruns);
        
        Ok(())
    }
}

/// Command for the ZMQ thread
enum ZmqCommand {
    Transmit(IqBuffer),
    HandleTxRequest,
    Receive(usize),
    GetStats,
    Shutdown,
}

/// Response from the ZMQ thread
enum ZmqResponse {
    TransmitResult(Result<(), InterfaceError>),
    HandleTxRequestResult(Result<(), InterfaceError>),  // Used by ZMQ thread
    ReceiveResult(Result<Option<IqBuffer>, InterfaceError>),
    Stats(RfStats),
    ShutdownComplete,
}

/// Async RF interface with TX/RX loops
pub struct AsyncZmqRf {
    command_tx: mpsc::Sender<(ZmqCommand, tokio::sync::oneshot::Sender<ZmqResponse>)>,
    tx_queue: mpsc::Sender<IqBuffer>,
    rx_queue: mpsc::Receiver<IqBuffer>,
    zmq_handle: Option<std::thread::JoinHandle<()>>,
    worker_handle: Option<tokio::task::JoinHandle<()>>,
}

/// A cloneable handle for sending samples to the RF interface
#[derive(Clone)]
pub struct ZmqRfSender {
    tx_queue: mpsc::Sender<IqBuffer>,
}

impl AsyncZmqRf {
    /// Create a new async ZMQ RF interface
    pub async fn new(config: ZmqRfConfig) -> Result<Self, InterfaceError> {
        // Create command channel for ZMQ thread
        let (command_tx, mut command_rx) = mpsc::channel::<(ZmqCommand, tokio::sync::oneshot::Sender<ZmqResponse>)>(100);
        
        // Create queues
        let (tx_sender, mut tx_receiver) = mpsc::channel::<IqBuffer>(config.ring_buffer_size);
        let (rx_sender, rx_receiver) = mpsc::channel::<IqBuffer>(config.ring_buffer_size);
        
        // Create a channel to report initialization result
        let (init_tx, init_rx) = std::sync::mpsc::channel::<Result<(), InterfaceError>>();
        
        // Spawn ZMQ thread
        let zmq_config = config.clone();
        let zmq_handle = std::thread::spawn(move || {
            // Create tokio runtime for async operations
            let rt = match tokio::runtime::Runtime::new() {
                Ok(rt) => rt,
                Err(e) => {
                    error!("Failed to create tokio runtime: {}", e);
                    let _ = init_tx.send(Err(InterfaceError::InitializationFailed(
                        format!("Failed to create tokio runtime: {}", e)
                    )));
                    return;
                }
            };
            
            rt.block_on(async {
                // Try to create and initialize driver
                let mut driver = match ZmqRfDriver::new(zmq_config) {
                    Ok(driver) => driver,
                    Err(e) => {
                        error!("Failed to create ZMQ driver: {:?}", e);
                        let _ = init_tx.send(Err(e));
                        return;
                    }
                };
                
                // Try to initialize with retries for address in use
                let mut retry_count = 0;
                const MAX_RETRIES: u32 = 3;
                const RETRY_DELAY_MS: u64 = 1000;
                
                loop {
                    match driver.initialize().await {
                        Ok(_) => {
                            info!("ZMQ driver initialized successfully");
                            let _ = init_tx.send(Ok(()));
                            break;
                        }
                        Err(e) => {
                            if retry_count < MAX_RETRIES {
                                error!("Failed to initialize ZMQ driver (attempt {}/{}): {:?}", 
                                    retry_count + 1, MAX_RETRIES, e);
                                retry_count += 1;
                                tokio::time::sleep(Duration::from_millis(RETRY_DELAY_MS)).await;
                            } else {
                                error!("Failed to initialize ZMQ driver after {} attempts: {:?}", 
                                    MAX_RETRIES, e);
                                let _ = init_tx.send(Err(e));
                                return;
                            }
                        }
                    }
                }
                
                // Process commands
                while let Some((cmd, response_tx)) = command_rx.recv().await {
                    let response = match cmd {
                        ZmqCommand::Transmit(buffer) => {
                            ZmqResponse::TransmitResult(driver.transmit(&buffer).await)
                        }
                        ZmqCommand::HandleTxRequest => {
                            ZmqResponse::HandleTxRequestResult(driver.handle_tx_request().await)
                        }
                        ZmqCommand::Receive(channel) => {
                            ZmqResponse::ReceiveResult(driver.receive(channel).await)
                        }
                        ZmqCommand::GetStats => {
                            ZmqResponse::Stats(driver.stats().await)
                        }
                        ZmqCommand::Shutdown => {
                            let _ = driver.shutdown().await;
                            ZmqResponse::ShutdownComplete
                        }
                    };
                    let _ = response_tx.send(response);
                }
            });
        });
        
        // Wait for initialization result
        match init_rx.recv_timeout(std::time::Duration::from_secs(10)) {
            Ok(Ok(())) => {
                // Initialization successful
            }
            Ok(Err(e)) => {
                error!("ZMQ initialization failed: {:?}", e);
                return Err(e);
            }
            Err(_) => {
                error!("ZMQ initialization timeout");
                return Err(InterfaceError::InitializationFailed(
                    "ZMQ initialization timeout".to_string()
                ));
            }
        }
        
        // Start worker task for TX/RX processing (srsRAN-compatible continuous loop)
        let cmd_tx = command_tx.clone();
        let worker_handle = tokio::spawn(async move {
            info!("ZMQ worker thread started, starting TX/RX processing immediately");
            // Start processing immediately to avoid dropping SSB samples
            // The UE will connect when ready
            
            let mut next_rx_time = Instant::now();
            let rx_period = Duration::from_millis(10); // Only attempt RX every 10ms
            let tx_check_period = Duration::from_micros(100); // Check for TX requests very frequently
            let mut next_tx_check = Instant::now();
            let mut tx_request_counter = 0u64;
            let mut rx_attempt_counter = 0u64;
            
            loop {
                // Handle all pending TX buffers first - drain the queue
                // This prevents the PHY->RF channel from backing up
                let mut buffers_processed = 0;
                let mut buffers_pending = 0;
                // Process ALL pending buffers without limit (like srsRAN)
                while let Ok(buffer) = tx_receiver.try_recv() {
                    buffers_pending += 1;
                    let (resp_tx, resp_rx) = tokio::sync::oneshot::channel();
                    if cmd_tx.send((ZmqCommand::Transmit(buffer), resp_tx)).await.is_err() {
                        break;
                    }
                    let _ = resp_rx.await; // Ignore result, buffer is now in circular buffer
                    buffers_processed += 1;
                }
                
                // Log if we processed many buffers
                if buffers_processed > 100 {
                    debug!("ZMQ worker: Processed {} buffers in one iteration", buffers_processed);
                }
                
                // CRITICAL: Always check for TX requests (like srsRAN)
                // This is the key difference - we check continuously, not just when data is pending
                let (resp_tx, resp_rx) = tokio::sync::oneshot::channel();
                if cmd_tx.send((ZmqCommand::HandleTxRequest, resp_tx)).await.is_err() {
                    break;
                }
                
                tx_request_counter += 1;
                
                // Check result but don't block on it
                match tokio::time::timeout(Duration::from_micros(100), resp_rx).await {
                    Ok(Ok(ZmqResponse::HandleTxRequestResult(Ok(_)))) => {
                        // Successfully handled a request
                        info!("ZMQ worker: Successfully handled TX request from UE!");
                        
                        // Log timing info for successful TX
                        static mut LAST_SUCCESS_TIME: Option<tokio::time::Instant> = None;
                        let now = tokio::time::Instant::now();
                        unsafe {
                            if let Some(last) = LAST_SUCCESS_TIME {
                                let delta = now.duration_since(last);
                                debug!("ZMQ worker: Time since last successful TX: {:?}", delta);
                            }
                            LAST_SUCCESS_TIME = Some(now);
                        }
                    }
                    Ok(Ok(ZmqResponse::HandleTxRequestResult(Err(_)))) => {
                        // Error handling request - this is normal when no UE request
                    }
                    _ => {
                        // Timeout or channel error, continue
                    }
                }
                
                // Log worker status periodically
                if tx_request_counter % 10000 == 0 {
                    debug!("ZMQ worker: Checked for TX requests {} times, RX attempts: {}", 
                           tx_request_counter, rx_attempt_counter);
                }
                
                // Handle RX at regular intervals
                // Note: For REQ-REP pattern, we should only attempt receive when we expect data
                // to avoid flooding with requests
                if Instant::now() >= next_rx_time {
                    next_rx_time += rx_period;  // Use longer period to avoid flooding
                    rx_attempt_counter += 1;
                    
                    if rx_attempt_counter % 1000 == 0 {
                        debug!("ZMQ worker: RX attempt #{}", rx_attempt_counter);
                    }
                    
                    for channel in 0..config.num_channels {
                        let (resp_tx, resp_rx) = tokio::sync::oneshot::channel();
                        if cmd_tx.send((ZmqCommand::Receive(channel), resp_tx)).await.is_err() {
                            break;
                        }
                        
                        match tokio::time::timeout(Duration::from_millis(5), resp_rx).await {
                            Ok(Ok(ZmqResponse::ReceiveResult(Ok(Some(buffer))))) => {
                                info!("ZMQ worker: Received {} samples from UE on channel {}", 
                                     buffer.samples.len(), channel);
                                if rx_sender.send(buffer).await.is_err() {
                                    break;
                                }
                            }
                            Ok(Ok(ZmqResponse::ReceiveResult(Err(e)))) => {
                                // RX errors are expected if no peer
                                if rx_attempt_counter == 1 {
                                    debug!("ZMQ worker: First RX attempt failed: {:?}", e);
                                }
                            }
                            _ => {}
                        }
                    }
                }
                
                // Only sleep if we didn't process any buffers AND didn't handle a TX request
                // This allows maximum throughput when there's work to do
                if buffers_processed == 0 {
                    tokio::time::sleep(Duration::from_micros(1)).await;
                }
            }
        });
        
        Ok(Self {
            command_tx,
            tx_queue: tx_sender,
            rx_queue: rx_receiver,
            zmq_handle: Some(zmq_handle),
            worker_handle: Some(worker_handle),
        })
    }
    
    /// Send samples for transmission
    pub async fn send(&self, buffer: IqBuffer) -> Result<(), InterfaceError> {
        // DEBUG: Check what we're sending
        let non_zero_count = buffer.samples.iter().filter(|s| s.norm() > 0.0).count();
        debug!("AsyncZmqRf::send() called with {} non-zero samples out of {}", 
               non_zero_count, buffer.samples.len());
               
        self.tx_queue.send(buffer).await
            .map_err(|_| InterfaceError::ConnectionFailed("TX queue closed".to_string()))
    }
    
    /// Receive samples
    pub async fn recv(&mut self) -> Option<IqBuffer> {
        self.rx_queue.recv().await
    }
    
    /// Get a cloneable sender handle for transmission
    pub fn get_sender(&self) -> ZmqRfSender {
        ZmqRfSender {
            tx_queue: self.tx_queue.clone(),
        }
    }
    
    /// Get RF statistics
    pub async fn stats(&self) -> RfStats {
        let (resp_tx, resp_rx) = tokio::sync::oneshot::channel();
        if self.command_tx.send((ZmqCommand::GetStats, resp_tx)).await.is_err() {
            return RfStats::default();
        }
        
        match resp_rx.await {
            Ok(ZmqResponse::Stats(stats)) => stats,
            _ => RfStats::default(),
        }
    }
    
    /// Shutdown the RF interface
    pub async fn shutdown(mut self) -> Result<(), InterfaceError> {
        // Send shutdown command
        let (resp_tx, resp_rx) = tokio::sync::oneshot::channel();
        let _ = self.command_tx.send((ZmqCommand::Shutdown, resp_tx)).await;
        let _ = resp_rx.await;
        
        // Stop worker task
        if let Some(handle) = self.worker_handle.take() {
            handle.abort();
        }
        
        // Wait for ZMQ thread to finish
        if let Some(handle) = self.zmq_handle.take() {
            let _ = handle.join();
        }
        
        Ok(())
    }
}

impl ZmqRfSender {
    /// Send samples for transmission
    pub async fn send(&self, buffer: IqBuffer) -> Result<(), InterfaceError> {
        // DEBUG: Check what we're sending
        let non_zero_count = buffer.samples.iter().filter(|s| s.norm() > 0.0).count();
        debug!("ZmqRfSender::send() called with {} non-zero samples out of {}", 
               non_zero_count, buffer.samples.len());
               
        self.tx_queue.send(buffer).await
            .map_err(|_| InterfaceError::ConnectionFailed("TX queue closed".to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_iq_buffer_serialization() {
        let buffer = IqBuffer::from_samples(
            vec![
                Complex32::new(1.0, 0.0),
                Complex32::new(0.0, 1.0),
                Complex32::new(-1.0, 0.0),
                Complex32::new(0.0, -1.0),
            ],
            12345,
            0,
        );
        
        // Test conversion to/from bytes
        let bytes = iq_buffer_to_bytes(&buffer);
        let buffer2 = bytes_to_iq_buffer(&bytes, buffer.timestamp, buffer.channel).unwrap();
        
        assert_eq!(buffer.timestamp, buffer2.timestamp);
        assert_eq!(buffer.samples.len(), buffer2.samples.len());
        for (a, b) in buffer.samples.iter().zip(buffer2.samples.iter()) {
            assert!((a.re - b.re).abs() < 1e-6);
            assert!((a.im - b.im).abs() < 1e-6);
        }
    }
    
    #[tokio::test]
    async fn test_rf_config() {
        let config = ZmqRfConfig::default();
        assert_eq!(config.sample_rate, DEFAULT_SAMPLE_RATE);
        assert_eq!(config.num_channels, 1);
    }
}