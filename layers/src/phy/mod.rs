//! Physical Layer (PHY) Submodules
//! 
//! This module contains the implementation of the 5G NR physical layer
//! according to 3GPP TS 38.201-38.215.

pub mod frame_structure;
pub mod resource_grid;
pub mod ofdm;
pub mod pss_sss;
pub mod pbch;
pub mod polar;
pub mod ldpc;
pub mod pdcch;
pub mod pdsch;
pub mod prach;
pub mod dmrs;

// Re-export commonly used types
pub use frame_structure::{FrameStructure, SlotConfig, SymbolType};
pub use resource_grid::{ResourceGrid, ResourceElement};
pub use ofdm::{OfdmModulator, OfdmDemodulator};
pub use pss_sss::{PssGenerator, SssGenerator, CellSearchResult};
pub use pbch::{PbchProcessor, Mib};
pub use pdcch::{PdcchProcessor, DciFormat10SiRnti};
pub use pdsch::{PdschProcessor, PdschConfig};
pub use prach::{PrachDetector, PrachDetectionResult, RachConfigCommon};

use crate::{LayerError, mac::MacPhyInterface};
use common::types::{Bandwidth, SubcarrierSpacing, Pci, CellId};
use interfaces::zmq_rf::{AsyncZmqRf, IqBuffer, ZmqRfConfig};
use num_complex::Complex32;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use tracing::{debug, error, info, warn};

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
    pdcch_processor: PdcchProcessor,
    pdsch_processor: PdschProcessor,
    prach_detector: Arc<Mutex<PrachDetector>>,
    state: Arc<RwLock<PhyState>>,
    running: Arc<RwLock<bool>>,
    initialized: bool,
    /// Channel for sending samples to RF interface
    rf_tx_channel: Option<tokio::sync::mpsc::Sender<IqBuffer>>,
    /// MAC-PHY interface for scheduling
    mac_interface: Option<Arc<dyn MacPhyInterface>>,
    /// Pre-computed PSS sequence (doesn't change)
    pss_sequence_precomputed: Vec<Complex32>,
    /// Pre-computed SSS sequences (for even and odd frames)
    sss_sequence_even_frame: Vec<Complex32>,
    sss_sequence_odd_frame: Vec<Complex32>,
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
        let mut ofdm_modulator = OfdmModulator::new(
            fft_size,
            config.cyclic_prefix,
            config.subcarrier_spacing,
        )?;
        
        // Configure OFDM modulator with proper bandwidth and baseband gain
        // Calculate bandwidth in RBs
        let bw_rb = match config.bandwidth {
            Bandwidth::Bw5 => 25,
            Bandwidth::Bw10 => 52,  // Band 3 with 15 kHz SCS
            Bandwidth::Bw15 => 79,
            Bandwidth::Bw20 => 106,
            Bandwidth::Bw25 => 133,
            Bandwidth::Bw30 => 160,
            Bandwidth::Bw40 => 216,
            Bandwidth::Bw50 => 270,
            Bandwidth::Bw60 => match config.subcarrier_spacing {
                SubcarrierSpacing::Scs15 => return Err(LayerError::InvalidConfiguration("60 MHz not supported with 15 kHz SCS".to_string())),
                SubcarrierSpacing::Scs30 => 162,
                SubcarrierSpacing::Scs60 => 81,
                _ => return Err(LayerError::InvalidConfiguration("Invalid SCS for 60 MHz".to_string())),
            },
            Bandwidth::Bw80 => match config.subcarrier_spacing {
                SubcarrierSpacing::Scs15 => return Err(LayerError::InvalidConfiguration("80 MHz not supported with 15 kHz SCS".to_string())),
                SubcarrierSpacing::Scs30 => 217,
                SubcarrierSpacing::Scs60 => 108,
                _ => return Err(LayerError::InvalidConfiguration("Invalid SCS for 80 MHz".to_string())),
            },
            Bandwidth::Bw100 => match config.subcarrier_spacing {
                SubcarrierSpacing::Scs15 => return Err(LayerError::InvalidConfiguration("100 MHz not supported with 15 kHz SCS".to_string())),
                SubcarrierSpacing::Scs30 => 273,
                SubcarrierSpacing::Scs60 => 135,
                _ => return Err(LayerError::InvalidConfiguration("Invalid SCS for 100 MHz".to_string())),
            },
        };
        
        // Configure with srsRAN-compatible baseband gain
        // Using 12 dB backoff as default (same as srsRAN)
        let baseband_backoff_db = 12.0;
        ofdm_modulator.configure_bandwidth(bw_rb, baseband_backoff_db);
        info!("Configured OFDM modulator: bw_rb={}, baseband_backoff_db={} dB", bw_rb, baseband_backoff_db);
        
        let ofdm_demodulator = OfdmDemodulator::new(
            fft_size,
            config.cyclic_prefix,
            config.subcarrier_spacing,
        )?;
        
        // Create synchronization signal generators with higher amplitude for better detection
        // Use 20 dB gain for PSS to ensure UE can detect it during cell search
        let pss_generator = PssGenerator::new_with_amplitude_db(config.pci, 20.0)?;
        let sss_generator = SssGenerator::new(config.pci)?;
        
        // Create PBCH processor
        let pbch_processor = PbchProcessor::new(config.pci, config.cell_id)?;
        
        // Create cell configuration for PDCCH/PDSCH
        let cell_config = Arc::new(common::CellConfig {
            pci: config.pci.0,
            cell_id: config.cell_id.0,
            bandwidth: config.bandwidth,
            subcarrier_spacing: config.subcarrier_spacing,
        });
        
        // Create PDCCH processor
        let pdcch_processor = PdcchProcessor::new(cell_config.clone());
        
        // Create PDSCH processor
        let pdsch_processor = PdschProcessor::new(cell_config);
        
        // Create PRACH detector with default RACH configuration
        let rach_config = RachConfigCommon::default();
        let prach_detector = PrachDetector::new(config.cell_id, rach_config)?;
        
        // Pre-compute PSS sequence (doesn't change)
        let pss_sequence_precomputed = pss_generator.generate();
        info!("Pre-computed PSS sequence with {} samples", pss_sequence_precomputed.len());
        
        // Pre-compute SSS sequences for even and odd frames
        let sss_sequence_even_frame = sss_generator.generate(0); // Even frame
        let sss_sequence_odd_frame = sss_generator.generate(1);  // Odd frame
        info!("Pre-computed SSS sequences for even/odd frames");
        
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
            pdcch_processor,
            pdsch_processor,
            prach_detector: Arc::new(Mutex::new(prach_detector)),
            state: Arc::new(RwLock::new(PhyState::new())),
            running: Arc::new(RwLock::new(false)),
            initialized: false,
            rf_tx_channel: None,
            mac_interface: None,
            pss_sequence_precomputed,
            sss_sequence_even_frame,
            sss_sequence_odd_frame,
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
        let pbch_processor = self.pbch_processor.clone();
        let pdcch_processor = self.pdcch_processor.clone();
        let pdsch_processor = self.pdsch_processor.clone();
        debug!("Getting RF TX channel");
        let rf_tx_channel = self.rf_tx_channel.as_ref().unwrap().clone();
        debug!("RF TX channel obtained");
        let config = self.config.clone();
        let mac_interface = self.mac_interface.clone();
        // Clone pre-computed sequences
        let pss_sequence = self.pss_sequence_precomputed.clone();
        let sss_sequence_even = self.sss_sequence_even_frame.clone();
        let sss_sequence_odd = self.sss_sequence_odd_frame.clone();
        
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
            // Symbol timing: samples_per_symbol={samples_per_symbol}, duration={symbol_duration:?}
            
            while *running.read().await {
                // Process all symbols in a slot as a batch for better timing
                for _symbol_in_slot in 0..symbols_per_slot {
                    let mut state_guard = state.write().await;
                    
                    // Get current timing
                    let frame = state_guard.frame_number;
                    let slot = state_guard.slot_number;
                    let symbol = state_guard.symbol_number;
                    
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
                        // OPTIMIZATION: Using pre-computed PSS sequence
                        if frame_structure.is_pss_symbol(symbol) {
                            {
                                let mut grid = resource_grid.lock().await;
                                let _ = grid.map_pss(symbol, &pss_sequence);
                            }
                        }
                        
                        // OPTIMIZATION: Using pre-computed SSS sequences
                        if frame_structure.is_sss_symbol(symbol) {
                            let sss_symbols = if frame % 2 == 0 {
                                &sss_sequence_even
                            } else {
                                &sss_sequence_odd
                            };
                            {
                                let mut grid = resource_grid.lock().await;
                                let _ = grid.map_sss(symbol, sss_symbols);
                            }
                        }
                    }
                    
                    // Map PBCH based on MAC scheduling or fallback
                    let should_send_pbch = slot_schedule.as_ref()
                        .and_then(|s| s.ssb_info.as_ref())
                        .map(|_ssb| frame_structure.is_pbch_symbol(frame, slot, symbol))
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
                        // PBCH with DMRS mapped
                    }
                    
                    // Map SIB1 if scheduled by MAC
                    if let Some(schedule) = &slot_schedule {
                        if let Some(sib1_info) = &schedule.sib1_info {
                            // Map PDCCH for SIB1 (only in first symbol of CORESET)
                            if symbol == sib1_info.coreset.start_symbol {
                                // Create DCI Format 1_0 for SI-RNTI
                                let dci_1_0 = DciFormat10SiRnti {
                                    frequency_resource: sib1_info.frequency_domain_assignment,
                                    time_resource: sib1_info.time_domain_assignment,
                                    vrb_to_prb_mapping: 0, // Non-interleaved
                                    modulation_coding_scheme: sib1_info.mcs_index,
                                    redundancy_version: 0,
                                    system_information_indicator: 0, // SIB1
                                };
                                
                                // Process PDCCH
                                {
                                    let mut grid = resource_grid.lock().await;
                                    pdcch_processor.process_sib1_pdcch(
                                        &mut *grid,
                                        &sib1_info.coreset,
                                        &dci_1_0,
                                        sib1_info.aggregation_level,
                                        sib1_info.cce_index,
                                    );
                                }
                                // PDCCH for SIB1 mapped
                            }
                            
                            // Map PDSCH for SIB1 data
                            let sib1_start = sib1_info.pdsch_time_alloc.start_symbol;
                            let sib1_length = sib1_info.pdsch_time_alloc.num_symbols;
                            if symbol >= sib1_start && symbol < sib1_start + sib1_length {
                                // Get SIB1 payload from MAC
                                if let Some(mac) = &mac_interface {
                                    match mac.get_sib1_payload().await {
                                        Ok(sib1_payload) => {
                                            // Create PDSCH configuration
                                            let pdsch_config = PdschConfig {
                                                tbs_bytes: sib1_info.tbs_bytes,
                                                modulation: sib1_info.modulation,
                                                num_layers: 1,
                                                rv: 0,
                                                ldpc_base_graph: if sib1_info.tbs_bytes > 292 { 1 } else { 2 },
                                                ndi: true,
                                                harq_id: 0,
                                                prb_allocation: sib1_info.prb_allocation.clone(),
                                                start_symbol: sib1_start,
                                                num_symbols: sib1_length,
                                                dmrs_type: 0,
                                                dmrs_additional_pos: 0,
                                                dmrs_config_type: 0,
                                                n_id: config.pci.0,
                                                rnti: 0xFFFF, // SI-RNTI
                                                code_block_size: (sib1_info.tbs_bytes + 3) * 8, // TBS + CRC in bits
                                            };
                                            
                                            // Process PDSCH
                                            {
                                                let mut grid = resource_grid.lock().await;
                                                pdsch_processor.process_sib1_pdsch(
                                                    &mut *grid,
                                                    &sib1_payload,
                                                    &pdsch_config,
                                                );
                                            }
                                            // PDSCH for SIB1 mapped
                                        }
                                        Err(e) => {
                                            error!("Failed to get SIB1 payload from MAC: {}", e);
                                        }
                                    }
                                }
                            }
                        }
                    }
                    
                    // OPTIMIZATION: Removed continuous signal transmission for performance
                    // The UE should be able to detect the cell with just SSB and SIB1 transmissions
                    
                    // OFDM modulation
                    let time_samples = {
                        let grid = resource_grid.lock().await;
                        let mut samples = ofdm_modulator.modulate(&*grid, symbol);
                        // Ensure correct number of samples
                        samples.resize(samples_per_symbol, num_complex::Complex32::new(0.0, 0.0));
                        samples
                    };
                    
                    // Create IQ buffer
                    let timestamp = state_guard.sample_count;
                    let iq_buffer = IqBuffer::from_samples(time_samples, timestamp, 0);
                    
                    // Send samples to RF channel - use blocking send to apply backpressure
                    // This prevents dropping samples and ensures timing synchronization
                    if let Err(e) = rf_tx_channel.send(iq_buffer).await {
                        error!("PHY: Failed to send samples to RF channel: {}", e);
                        break;
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
        let state = self.state.clone();
        let prach_detector = self.prach_detector.clone();
        let mac_interface = self.mac_interface.clone();
        // We don't clone RF interface, just check if it exists
        let frame_structure = self.frame_structure.clone();
        
        tokio::spawn(async move {
            info!("Uplink processing task started");
            
            // Use similar timing as downlink
            let mut next_slot_time = tokio::time::Instant::now();
            let slot_duration = frame_structure.slot_duration();
            let slots_per_frame = frame_structure.slots_per_frame();
            
            while *running.read().await {
                // Get current timing
                let (frame, slot) = {
                    let state_guard = state.read().await;
                    (state_guard.frame_number, state_guard.slot_number)
                };
                
                // Check if this is a PRACH occasion
                let is_prach_slot = {
                    let mut detector = prach_detector.lock().await;
                    detector.is_prach_occasion(frame, slot)
                };
                
                if is_prach_slot {
                    debug!("PRACH occasion at frame={}, slot={}", frame, slot);
                    
                    // For now, simulate PRACH detection since RF receiver is not implemented
                    // In a real implementation, we would receive samples from RF
                    {
                        // For now, we simulate PRACH detection since uplink is not fully implemented
                        // In a real implementation, we would:
                        // 1. Receive samples from RF
                        // 2. Pass them to PRACH detector
                        // 3. Report detections to MAC
                        
                        // Simulate receiving samples (placeholder)
                        let dummy_samples = vec![num_complex::Complex32::new(0.0, 0.0); 30720];
                        
                        // Detect PRACH preambles
                        let detection_result = {
                            let mut detector = prach_detector.lock().await;
                            match detector.detect(&dummy_samples, frame, slot) {
                                Ok(result) => result,
                                Err(e) => {
                                    error!("PRACH detection failed: {}", e);
                                    continue;
                                }
                            }
                        };
                        
                        // Report to MAC if preambles detected
                        if !detection_result.preambles.is_empty() {
                            if let Some(mac) = &mac_interface {
                                if let Err(e) = mac.report_prach_detection(detection_result).await {
                                    error!("Failed to report PRACH detection to MAC: {}", e);
                                }
                            }
                        }
                    }
                }
                
                // Wait until next slot
                next_slot_time += slot_duration;
                let now = tokio::time::Instant::now();
                if next_slot_time > now {
                    tokio::time::sleep_until(next_slot_time).await;
                } else {
                    // Running behind
                    next_slot_time = now;
                }
            }
            
            info!("Uplink processing stopped");
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