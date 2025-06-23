//! Physical Layer (PHY) Submodules
//! 
//! This module contains the implementation of the 5G NR physical layer
//! according to 3GPP TS 38.201-38.215.

pub mod frame_structure;
pub mod resource_grid;
pub mod ofdm;
pub mod pss_sss;
pub mod pbch;

// Re-export commonly used types
pub use frame_structure::{FrameStructure, SlotConfig, SymbolType};
pub use resource_grid::{ResourceGrid, ResourceElement};
pub use ofdm::{OfdmModulator, OfdmDemodulator};
pub use pss_sss::{PssGenerator, SssGenerator, CellSearchResult};
pub use pbch::{PbchProcessor, Mib};

use crate::{LayerError, mac::MacPhyInterface};
use common::types::{Bandwidth, SubcarrierSpacing, Pci, CellId};
use interfaces::zmq_rf::{AsyncZmqRf, IqBuffer, ZmqRfConfig};
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use tracing::{debug, error, info, warn, trace};

/// PHY layer configuration
#[derive(Debug, Clone)]
pub struct PhyConfig {
    /// Physical cell ID
    pub pci: Pci,
    /// Cell ID
    pub cell_id: CellId,
    /// Carrier frequency in Hz
    pub carrier_frequency: f64,
    /// Bandwidth
    pub bandwidth: Bandwidth,
    /// Subcarrier spacing
    pub subcarrier_spacing: SubcarrierSpacing,
    /// Number of TX antennas
    pub num_tx_antennas: usize,
    /// Number of RX antennas
    pub num_rx_antennas: usize,
    /// Cyclic prefix type (normal/extended)
    pub cyclic_prefix: CyclicPrefix,
    /// Frame structure (FDD/TDD)
    pub duplex_mode: DuplexMode,
}

/// Cyclic prefix type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CyclicPrefix {
    Normal,
    Extended,
}

/// Duplex mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DuplexMode {
    Fdd,
    Tdd { pattern: TddPattern },
}

/// TDD pattern
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TddPattern {
    pub dl_slots: u8,
    pub ul_slots: u8,
    pub special_slots: u8,
}

/// PHY processing state
#[derive(Debug)]
struct PhyState {
    frame_number: u32,
    slot_number: u8,
    symbol_number: u8,
    sample_count: u64,
}

impl PhyState {
    fn new() -> Self {
        Self {
            frame_number: 0,
            slot_number: 0,
            symbol_number: 0,
            sample_count: 0,
        }
    }
    
    fn advance_symbol(&mut self, symbols_per_slot: u8, slots_per_frame: u8) {
        self.symbol_number += 1;
        if self.symbol_number >= symbols_per_slot {
            self.symbol_number = 0;
            self.slot_number += 1;
            if self.slot_number >= slots_per_frame {
                self.slot_number = 0;
                self.frame_number = (self.frame_number + 1) % 1024;
            }
        }
    }
}

/// Enhanced PHY layer implementation
pub struct EnhancedPhyLayer {
    config: PhyConfig,
    rf_interface: Option<AsyncZmqRf>,
    frame_structure: FrameStructure,
    resource_grid: Arc<Mutex<ResourceGrid>>,
    ofdm_modulator: OfdmModulator,
    ofdm_demodulator: OfdmDemodulator,
    pss_generator: PssGenerator,
    sss_generator: SssGenerator,
    pbch_processor: PbchProcessor,
    state: Arc<RwLock<PhyState>>,
    running: Arc<RwLock<bool>>,
    initialized: bool,
    /// Channel for sending samples to RF interface
    rf_tx_channel: Option<tokio::sync::mpsc::Sender<IqBuffer>>,
    /// MAC-PHY interface for scheduling
    mac_interface: Option<Arc<dyn MacPhyInterface>>,
}

impl EnhancedPhyLayer {
    /// Create a new enhanced PHY layer
    pub fn new(config: PhyConfig) -> Result<Self, LayerError> {
        // Calculate FFT size based on bandwidth and SCS
        let fft_size = calculate_fft_size(&config.bandwidth, &config.subcarrier_spacing)?;
        
        // Create frame structure
        let mut frame_structure = FrameStructure::new(
            config.subcarrier_spacing,
            config.cyclic_prefix,
        );
        // Set duplex mode
        frame_structure.set_duplex_mode(config.duplex_mode);
        
        // Create resource grid
        let resource_grid = ResourceGrid::new(
            fft_size,
            frame_structure.symbols_per_slot(),
            config.bandwidth,
            config.subcarrier_spacing,
        )?;
        
        // Create OFDM components
        let ofdm_modulator = OfdmModulator::new(
            fft_size,
            config.cyclic_prefix,
            config.subcarrier_spacing,
        )?;
        
        let ofdm_demodulator = OfdmDemodulator::new(
            fft_size,
            config.cyclic_prefix,
            config.subcarrier_spacing,
        )?;
        
        // Create synchronization signal generators
        let pss_generator = PssGenerator::new(config.pci)?;
        let sss_generator = SssGenerator::new(config.pci)?;
        
        // Create PBCH processor
        let pbch_processor = PbchProcessor::new(config.pci, config.cell_id)?;
        
        Ok(Self {
            config,
            rf_interface: None,
            frame_structure,
            resource_grid: Arc::new(Mutex::new(resource_grid)),
            ofdm_modulator,
            ofdm_demodulator,
            pss_generator,
            sss_generator,
            pbch_processor,
            state: Arc::new(RwLock::new(PhyState::new())),
            running: Arc::new(RwLock::new(false)),
            initialized: false,
            rf_tx_channel: None,
            mac_interface: None,
        })
    }
    
    /// Set MAC-PHY interface
    pub fn set_mac_interface(&mut self, mac_interface: Arc<dyn MacPhyInterface>) {
        self.mac_interface = Some(mac_interface);
        info!("MAC-PHY interface set");
    }
    
    /// Initialize with RF interface
    pub async fn initialize_with_rf(&mut self, rf_config: ZmqRfConfig) -> Result<(), LayerError> {
        info!("Initializing PHY layer with RF interface");
        
        // Create RF interface
        let rf_interface = AsyncZmqRf::new(rf_config).await
            .map_err(|e| LayerError::InitializationFailed(e.to_string()))?;
        
        // Get a sender handle for the RF interface
        let rf_sender = rf_interface.get_sender();
        
        // Create channel for RF transmission with massive buffer to prevent backpressure
        // This should be large enough to handle rate mismatches between PHY and RF
        let (tx_sender, mut tx_receiver) = tokio::sync::mpsc::channel::<IqBuffer>(16384);
        
        // Spawn task to handle RF transmission
        tokio::spawn(async move {
            while let Some(buffer) = tx_receiver.recv().await {
                if let Err(e) = rf_sender.send(buffer).await {
                    error!("Failed to send samples to RF: {}", e);
                }
            }
            info!("RF transmission task ended");
        });
        
        self.rf_interface = Some(rf_interface);
        self.rf_tx_channel = Some(tx_sender);
        self.initialized = true;
        *self.running.write().await = true;
        
        info!("PHY layer initialized with RF interface");
        Ok(())
    }
    
    /// Start PHY processing loops
    pub async fn start_processing(&self) -> Result<(), LayerError> {
        if !self.initialized {
            return Err(LayerError::NotInitialized);
        }
        
        info!("Starting PHY processing");
        
        // Start downlink processing
        let dl_handle = self.start_downlink_processing();
        
        // Start uplink processing
        let ul_handle = self.start_uplink_processing();
        
        // Wait for tasks
        tokio::select! {
            _ = dl_handle => {
                warn!("Downlink processing stopped");
            }
            _ = ul_handle => {
                warn!("Uplink processing stopped");
            }
        }
        
        Ok(())
    }
    
    /// Start downlink processing
    fn start_downlink_processing(&self) -> tokio::task::JoinHandle<()> {
        debug!("Starting downlink processing task");
        let running = self.running.clone();
        let state = self.state.clone();
        let frame_structure = self.frame_structure.clone();
        debug!("Getting resource grid reference");
        let resource_grid = self.resource_grid.clone();
        debug!("Resource grid reference obtained");
        let ofdm_modulator = self.ofdm_modulator.clone();
        let pss_generator = self.pss_generator.clone();
        let sss_generator = self.sss_generator.clone();
        let pbch_processor = self.pbch_processor.clone();
        debug!("Getting RF TX channel");
        let rf_tx_channel = self.rf_tx_channel.as_ref().unwrap().clone();
        debug!("RF TX channel obtained");
        let config = self.config.clone();
        let mac_interface = self.mac_interface.clone();
        
        tokio::spawn(async move {
            debug!("Downlink processing task started");
            
            // Use high-precision timing
            let mut next_symbol_time = tokio::time::Instant::now();
            let symbol_duration = frame_structure.symbol_duration();
            let symbols_per_slot = frame_structure.symbols_per_slot();
            let slots_per_frame = frame_structure.slots_per_frame();
            
            // Calculate samples per symbol based on actual sample rate and symbol duration
            // For band 3 FDD with 15 kHz SCS, we use 23.04 MHz sample rate
            let actual_sample_rate = if config.subcarrier_spacing == SubcarrierSpacing::Scs15 && config.bandwidth == Bandwidth::Bw20 {
                23.04e6  // Band 3 FDD specific
            } else {
                config.bandwidth.to_sample_rate()
            };
            let samples_per_symbol = ((actual_sample_rate * symbol_duration.as_secs_f64()) as usize + 1) & !1; // Round up to even
            info!("Samples per symbol: {}, Symbol duration: {:?}, Sample rate: {} MHz", 
                  samples_per_symbol, symbol_duration, actual_sample_rate / 1e6);
            
            while *running.read().await {
                // Process all symbols in a slot as a batch for better timing
                for _symbol_in_slot in 0..symbols_per_slot {
                    let mut state_guard = state.write().await;
                    
                    // Get current timing
                    let frame = state_guard.frame_number;
                    let slot = state_guard.slot_number;
                    let symbol = state_guard.symbol_number;
                    
                    trace!("DL processing: frame={}, slot={}, symbol={}", frame, slot, symbol);
                    
                    // Clear resource grid for this symbol
                    {
                        let mut grid = resource_grid.lock().await;
                        grid.clear_symbol(symbol);
                    }
                    
                    // Get scheduling information from MAC layer if available
                    let slot_schedule = if let Some(mac) = &mac_interface {
                        match mac.get_slot_schedule(frame, slot).await {
                            Ok(schedule) => Some(schedule),
                            Err(e) => {
                                debug!("Failed to get MAC schedule: {}, using fallback", e);
                                None
                            }
                        }
                    } else {
                        None
                    };
                    
                    // Map synchronization signals (PSS/SSS) based on MAC scheduling or fallback
                    let should_send_ssb = slot_schedule.as_ref()
                        .and_then(|s| s.ssb_info.as_ref())
                        .map(|ssb| symbol >= ssb.start_symbol && symbol < ssb.start_symbol + 4) // SSB spans 4 symbols
                        .unwrap_or_else(|| frame_structure.is_sync_symbol(frame, slot, symbol));
                    
                    if should_send_ssb {
                        debug!("Processing sync symbol: frame={}, slot={}, symbol={}", frame, slot, symbol);
                        if frame_structure.is_pss_symbol(symbol) {
                            let pss_symbols = pss_generator.generate();
                            {
                                let mut grid = resource_grid.lock().await;
                                let _ = grid.map_pss(symbol, &pss_symbols);
                            }
                            // DEBUG: Check if PSS was actually mapped
                            {
                                let grid = resource_grid.lock().await;
                                let symbol_data = grid.get_symbol(symbol);
                                let non_zero_count = symbol_data.iter().filter(|x| x.norm() > 0.0).count();
                                debug!("DEBUG: After PSS mapping, symbol {} has {} non-zero samples out of {}", 
                                      symbol, non_zero_count, symbol_data.len());
                            }
                            info!("Mapped PSS at frame={}, slot={}, symbol={} (SSB transmission)", frame, slot, symbol);
                        }
                        
                        if frame_structure.is_sss_symbol(symbol) {
                            let sss_symbols = sss_generator.generate(frame);
                            {
                                let mut grid = resource_grid.lock().await;
                                let _ = grid.map_sss(symbol, &sss_symbols);
                            }
                            info!("Mapped SSS at frame={}, slot={}, symbol={} (SSB transmission)", frame, slot, symbol);
                        }
                    }
                    
                    // Map PBCH based on MAC scheduling or fallback
                    let should_send_pbch = slot_schedule.as_ref()
                        .and_then(|s| s.ssb_info.as_ref())
                        .map(|ssb| frame_structure.is_pbch_symbol(frame, slot, symbol))
                        .unwrap_or_else(|| frame_structure.is_pbch_symbol(frame, slot, symbol));
                        
                    if should_send_pbch {
                        let mib = pbch_processor.generate_mib(frame);
                        let pbch_symbols = pbch_processor.encode_pbch(&mib, frame);
                        {
                            let mut grid = resource_grid.lock().await;
                            // Map PBCH data symbols
                            let _ = grid.map_pbch(symbol, &pbch_symbols);
                            // Map PBCH DMRS (crucial for demodulation)
                            let _ = grid.map_pbch_dmrs(symbol, config.cell_id.0);
                        }
                        info!("Mapped PBCH with DMRS at frame={}, slot={}, symbol={}", frame, slot, symbol);
                    }
                    
                    // Map SIB1 if scheduled by MAC
                    if let Some(schedule) = &slot_schedule {
                        if let Some(sib1_info) = &schedule.sib1_info {
                            let sib1_start = sib1_info.pdsch_time_alloc.start_symbol;
                            let sib1_length = sib1_info.pdsch_time_alloc.num_symbols;
                            if symbol >= sib1_start && symbol < sib1_start + sib1_length {
                                // Get SIB1 payload from MAC
                                if let Some(mac) = &mac_interface {
                                    match mac.get_sib1_payload().await {
                                        Ok(sib1_payload) => {
                                            // TODO: Implement SIB1 to PDSCH mapping
                                            info!("SIB1 scheduled at frame={}, slot={}, symbol={} (payload: {} bytes)", 
                                                  frame, slot, symbol, sib1_payload.len());
                                        }
                                        Err(e) => {
                                            error!("Failed to get SIB1 payload from MAC: {}", e);
                                        }
                                    }
                                }
                            }
                        }
                    }
                    
                    // OFDM modulation
                    let time_samples = {
                        let grid = resource_grid.lock().await;
                        let mut samples = ofdm_modulator.modulate(&*grid, symbol);
                        // Ensure correct number of samples
                        samples.resize(samples_per_symbol, num_complex::Complex32::new(0.0, 0.0));
                        
                        // DEBUG: Check modulated samples
                        let non_zero_count = samples.iter().filter(|x| x.norm() > 0.0).count();
                        debug!("DEBUG: OFDM modulated symbol {} has {} non-zero samples out of {}", 
                              symbol, non_zero_count, samples.len());
                        
                        samples
                    };
                    
                    // Create IQ buffer
                    let timestamp = state_guard.sample_count;
                    let iq_buffer = IqBuffer::from_samples(time_samples, timestamp, 0);
                    
                    // Send to RF through channel (non-blocking)
                    // DEBUG: Check buffer before sending
                    let sample_count = iq_buffer.samples.len();
                    let non_zero = iq_buffer.samples.iter().filter(|s| s.norm() > 0.0).count();
                    debug!("PHY: Sending {} samples ({} non-zero) to RF channel", sample_count, non_zero);
                    
                    // Send samples to RF channel - use blocking send to apply backpressure
                    // This prevents dropping samples and ensures timing synchronization
                    if let Err(e) = rf_tx_channel.send(iq_buffer).await {
                        error!("PHY: Failed to send samples to RF channel: {}", e);
                        break;
                    } else {
                        debug!("PHY: Successfully queued samples for transmission");
                    }
                    
                    // Update state
                    state_guard.sample_count += samples_per_symbol as u64;
                    state_guard.advance_symbol(symbols_per_slot, slots_per_frame);
                    
                    // Drop the state guard to avoid holding the lock
                    drop(state_guard);
                    
                    // Wait until next symbol time
                    next_symbol_time += symbol_duration;
                    let now = tokio::time::Instant::now();
                    if next_symbol_time > now {
                        tokio::time::sleep_until(next_symbol_time).await;
                    } else {
                        // We're running behind, log and continue
                        warn!("DL processing running behind by {:?}", now - next_symbol_time);
                        next_symbol_time = now;
                    }
                }
            }
        })
    }
    
    /// Start uplink processing
    fn start_uplink_processing(&self) -> tokio::task::JoinHandle<()> {
        let running = self.running.clone();
        
        tokio::spawn(async move {
            while *running.read().await {
                // TODO: Implement uplink processing with proper RF receiver handle
                // Currently only downlink is implemented
                
                // Small delay to prevent busy loop
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            }
            
            debug!("Uplink processing stopped");
        })
    }
    
    /// Stop PHY processing
    pub async fn stop_processing(&self) -> Result<(), LayerError> {
        info!("Stopping PHY processing");
        *self.running.write().await = false;
        Ok(())
    }
    
    /// Get PHY statistics
    pub async fn get_stats(&self) -> PhyStats {
        let state = self.state.read().await;
        let rf_stats = if let Some(rf) = &self.rf_interface {
            Some(rf.stats().await)
        } else {
            None
        };
        
        PhyStats {
            frame_number: state.frame_number,
            slot_number: state.slot_number,
            symbol_number: state.symbol_number,
            sample_count: state.sample_count,
            rf_stats,
        }
    }
}

/// PHY layer statistics
#[derive(Debug)]
pub struct PhyStats {
    pub frame_number: u32,
    pub slot_number: u8,
    pub symbol_number: u8,
    pub sample_count: u64,
    pub rf_stats: Option<interfaces::zmq_rf::RfStats>,
}

/// Calculate FFT size based on bandwidth and subcarrier spacing
fn calculate_fft_size(bandwidth: &Bandwidth, scs: &SubcarrierSpacing) -> Result<usize, LayerError> {
    let scs_khz = *scs as u32;
    let bw_mhz = match bandwidth {
        Bandwidth::Bw5 => 5,
        Bandwidth::Bw10 => 10,
        Bandwidth::Bw15 => 15,
        Bandwidth::Bw20 => 20,
        Bandwidth::Bw25 => 25,
        Bandwidth::Bw30 => 30,
        Bandwidth::Bw40 => 40,
        Bandwidth::Bw50 => 50,
        Bandwidth::Bw60 => 60,
        Bandwidth::Bw80 => 80,
        Bandwidth::Bw100 => 100,
    };
    
    // Calculate minimum FFT size
    let min_fft = (bw_mhz * 1000) / scs_khz;
    
    // Round up to next power of 2
    let fft_size = min_fft.next_power_of_two() as usize;
    
    // Common FFT sizes for 5G NR
    match fft_size {
        256 | 512 | 1024 | 2048 | 4096 => Ok(fft_size),
        _ => Err(LayerError::InvalidConfiguration(
            format!("Invalid FFT size {} for bandwidth {:?} and SCS {:?}", fft_size, bandwidth, scs)
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_fft_size_calculation() {
        // 20 MHz, 15 kHz SCS -> 2048 FFT
        let fft = calculate_fft_size(&Bandwidth::Bw20, &SubcarrierSpacing::Scs15).unwrap();
        assert_eq!(fft, 2048);
        
        // 100 MHz, 30 kHz SCS -> 4096 FFT
        let fft = calculate_fft_size(&Bandwidth::Bw100, &SubcarrierSpacing::Scs30).unwrap();
        assert_eq!(fft, 4096);
    }
}