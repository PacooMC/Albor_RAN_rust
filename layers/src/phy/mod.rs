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
pub mod resampler;

// Re-export commonly used types
pub use frame_structure::{FrameStructure, SlotConfig, SymbolType};
pub use resource_grid::{ResourceGrid, ResourceElement};
pub use ofdm::{OfdmModulator, OfdmDemodulator};
pub use pss_sss::{PssGenerator, SssGenerator, CellSearchResult};
pub use pbch::{PbchProcessor, Mib};
pub use pdcch::{PdcchProcessor, DciFormat10SiRnti};
pub use pdsch::{PdschProcessor, PdschConfig};
pub use prach::{PrachDetector, PrachDetectionResult, RachConfigCommon};
use resampler::{Resampler, ResamplerConfig};

use crate::{LayerError, mac::MacPhyInterface};
use common::types::{Bandwidth, SubcarrierSpacing, Pci, CellId};
use interfaces::zmq_rf::{AsyncZmqRf, IqBuffer, ZmqRfConfig};
use num_complex::Complex32;
use std::sync::Arc;
use std::collections::HashMap;
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
    /// SSB subcarrier offset (k_SSB) in subcarriers
    pub k_ssb: i16,
    /// Sample rate in Hz from configuration
    pub sample_rate: f64,
    /// PRACH configuration
    pub prach_config: prach::RachConfigCommon,
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
    /// Expected sample count based on perfect timing
    expected_sample_count: u64,
    /// Sample count at the start of current symbol
    symbol_start_sample: u64,
    /// Cached PBCH symbols per SSB index
    current_pbch_symbols: HashMap<u8, Vec<Complex32>>,
}

impl PhyState {
    fn new() -> Self {
        Self {
            frame_number: 0,
            slot_number: 0,
            symbol_number: 0,
            sample_count: 0,
            expected_sample_count: 0,
            symbol_start_sample: 0,
            current_pbch_symbols: HashMap::new(),
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
                // Clear PBCH cache for new frame
                self.current_pbch_symbols.clear();
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
    /// Sample rate resampler for PHY->ZMQ conversion
    resampler: Option<Arc<Mutex<Resampler>>>,
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
        
        // Create resource grid with cell ID
        let resource_grid = ResourceGrid::new_with_cell_id(
            fft_size,
            frame_structure.symbols_per_slot(),
            config.bandwidth,
            config.subcarrier_spacing,
            config.cell_id.0,
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
        
        // Configure baseband gain for optimal signal power
        // With FFT loss compensation in OFDM modulator, we need proper backoff
        // Target PSS power: -3 to -6 dB (with 3 dB PSS boost)
        // Balanced backoff to prevent saturation while maintaining detectable signal
        // CRITICAL: Increased to 40 dB to prevent saturation with sacred config tx_gain of 75 dB
        let baseband_backoff_db = 0.0;  // No backoff to maximize signal power for cell detection
        ofdm_modulator.configure_bandwidth(bw_rb, baseband_backoff_db);
        info!("Configured OFDM modulator: bw_rb={}, baseband_backoff_db={} dB", bw_rb, baseband_backoff_db);
        
        // TEST: Verify OFDM modulator works with simple test signal
        info!("Testing OFDM modulator with single tone...");
        let test_output = ofdm_modulator.test_ofdm_with_single_tone();
        let test_power: f32 = test_output.iter().map(|s| s.norm_sqr()).sum::<f32>() / test_output.len() as f32;
        let test_power_db = 10.0 * test_power.log10();
        info!("OFDM test result: output power = {:.2} dB, non-zero samples = {}", 
              test_power_db, test_output.iter().filter(|s| s.norm() > 0.0).count());
        if test_output.iter().all(|s| s.norm() == 0.0) {
            error!("ERROR: OFDM modulator test failed - all outputs are zero!");
            return Err(LayerError::InitializationFailed("OFDM modulator produces zero output".to_string()));
        }
        
        let ofdm_demodulator = OfdmDemodulator::new(
            fft_size,
            config.cyclic_prefix,
            config.subcarrier_spacing,
        )?;
        
        // Create synchronization signal generators with proper amplitude
        // PSS should be higher than SSS for better detection
        // Using 3 dB for PSS (same as srsRAN default)
        let pss_generator = PssGenerator::new_with_amplitude_db(config.pci, 3.0)?;
        let sss_generator = SssGenerator::new(config.pci)?;  // 0 dB (amplitude = 1.0)
        
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
        
        // Create PRACH detector with RACH configuration from config file
        let prach_detector = PrachDetector::new(config.cell_id, config.prach_config.clone())?;
        
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
            resampler: None, // Will be initialized when RF is configured
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
        
        // Calculate PHY natural sample rate (FFT size × SCS)
        let fft_size = calculate_fft_size(&self.config.bandwidth, &self.config.subcarrier_spacing)?;
        let phy_sample_rate = fft_size as f64 * (self.config.subcarrier_spacing as u32 * 1000) as f64;
        let rf_sample_rate = rf_config.sample_rate;
        
        info!("PHY natural sample rate: {} MHz", phy_sample_rate / 1e6);
        info!("RF configured sample rate: {} MHz", rf_sample_rate / 1e6);
        
        // Check if resampling is needed
        if (phy_sample_rate - rf_sample_rate).abs() > 1.0 {
            info!("Sample rate conversion needed: {} MHz -> {} MHz", 
                  phy_sample_rate / 1e6, rf_sample_rate / 1e6);
            
            // Create resampler configuration
            let resampler_config = ResamplerConfig {
                input_rate: phy_sample_rate,
                output_rate: rf_sample_rate,
                filter_order: 64,
                cutoff_factor: 0.45,
            };
            
            // Create resampler
            let resampler = Resampler::new(resampler_config);
            self.resampler = Some(Arc::new(Mutex::new(resampler)));
            
            info!("Resampler initialized for {}:{} rate conversion", 
                  (phy_sample_rate / rf_sample_rate * 4.0).round() as usize,
                  ((rf_sample_rate / phy_sample_rate) * 4.0).round() as usize);
        }
        
        // Create RF interface
        let rf_interface = AsyncZmqRf::new(rf_config).await
            .map_err(|e| LayerError::InitializationFailed(e.to_string()))?;
        
        // Get a sender handle for the RF interface
        let rf_sender = rf_interface.get_sender();
        
        // Create channel for RF transmission with massive buffer to prevent backpressure
        // This should be large enough to handle rate mismatches between PHY and RF
        let (tx_sender, mut tx_receiver) = tokio::sync::mpsc::channel::<IqBuffer>(16384);
        
        // Clone resampler for the forwarding task
        let resampler_clone = self.resampler.clone();
        
        // Spawn task to handle RF transmission with optional resampling
        tokio::spawn(async move {
            info!("RF transmission forwarding task started");
            let mut forward_count = 0u64;
            while let Some(mut buffer) = tx_receiver.recv().await {
                // Apply resampling if configured
                if let Some(resampler_arc) = &resampler_clone {
                    let mut resampler = resampler_arc.lock().await;
                    
                    // Process samples through resampler
                    let resampled = resampler.process(&buffer.samples);
                    
                    // Log resampling info periodically
                    let non_zero_before = buffer.samples.iter().filter(|s| s.norm() > 0.0).count();
                    let non_zero_after = resampled.iter().filter(|s| s.norm() > 0.0).count();
                    if non_zero_before > 0 || forward_count % 1000 == 0 {
                        info!("Resampling: {} samples -> {} samples ({} -> {} non-zero)", 
                              buffer.samples.len(), resampled.len(), non_zero_before, non_zero_after);
                    }
                    
                    // Update buffer with resampled data
                    buffer.samples = resampled;
                }
                
                // Debug logging to track sample flow
                let non_zero_count = buffer.samples.iter().filter(|s| s.norm() > 0.0).count();
                if non_zero_count > 0 || forward_count % 1000 == 0 {
                    info!("PHY->RF: Forwarding buffer #{} with {} non-zero samples out of {}", 
                          forward_count, non_zero_count, buffer.samples.len());
                }
                
                // CRITICAL FIX: Don't terminate on send failure - drop buffer and continue
                // This prevents the forwarding task from stopping when buffers back up
                if let Err(e) = rf_sender.send(buffer).await {
                    error!("Failed to send samples to RF (dropping buffer): {}", e);
                    // Don't break! Continue forwarding to maintain continuous operation
                    // The dropped samples will cause an underrun but that's better than stopping
                    static mut DROP_COUNT: u64 = 0;
                    unsafe {
                        DROP_COUNT += 1;
                        if DROP_COUNT % 100 == 1 {
                            warn!("PHY->RF: Dropped {} buffers total due to backpressure", DROP_COUNT);
                        }
                    }
                }
                forward_count += 1;
            }
            error!("RF transmission forwarding task ended after {} buffers!", forward_count);
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
        let resampler = self.resampler.clone();
        
        tokio::spawn(async move {
            debug!("Downlink processing task started");
            
            // PRE-BUFFERING: Generate initial samples before UE connects
            // This prevents the race condition where UE gets zeros
            info!("Pre-buffering samples to prevent ZMQ race condition...");
            
            // Use high-precision timing
            let mut next_symbol_time = tokio::time::Instant::now();
            let symbol_duration = frame_structure.symbol_duration();
            let symbols_per_slot = frame_structure.symbols_per_slot();
            let slots_per_frame = frame_structure.slots_per_frame();
            
            // Calculate natural sample rate from FFT size and subcarrier spacing
            // This ensures perfect alignment with OFDM processing
            let fft_size = calculate_fft_size(&config.bandwidth, &config.subcarrier_spacing)
                .expect("Failed to calculate FFT size");
            let scs_hz = match config.subcarrier_spacing {
                SubcarrierSpacing::Scs15 => 15_000.0,
                SubcarrierSpacing::Scs30 => 30_000.0,
                SubcarrierSpacing::Scs60 => 60_000.0,
                SubcarrierSpacing::Scs120 => 120_000.0,
                SubcarrierSpacing::Scs240 => 240_000.0,
            };
            let natural_sample_rate = fft_size as f64 * scs_hz;
            info!("Using natural sample rate: {} Hz (FFT size {} × SCS {} Hz)", 
                  natural_sample_rate, fft_size, scs_hz);
            
            // Calculate samples per symbol using natural rate
            let samples_per_symbol = ((natural_sample_rate * symbol_duration.as_secs_f64()) as usize + 1) & !1; // Round up to even
            
            // CRITICAL FIX: Reduce pre-buffering to align SSB timing with UE expectations
            // Pre-buffer only 20ms (one SSB period) to ensure UE receives SSBs at expected times
            // This ensures the first SSB the UE sees is at frame 0 or frame 2
            let pre_buffer_ms = 0.02; // 20ms = one SSB period
            let pre_buffer_symbols = (pre_buffer_ms / symbol_duration.as_secs_f64()) as usize;
            let mut pre_buffer_count = 0;
            
            info!("Pre-buffering {} symbols ({}ms - one SSB period) to align timing...", pre_buffer_symbols, pre_buffer_ms * 1000.0);
            
            // Pre-buffer loop - generate samples as fast as possible
            for _ in 0..pre_buffer_symbols {
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
                
                // Check if this is an SSB symbol
                let should_send_ssb = frame_structure.is_sync_symbol(frame, slot, symbol);
                
                if should_send_ssb {
                    // Map PSS
                    if frame_structure.is_pss_symbol(symbol) {
                        {
                            let mut grid = resource_grid.lock().await;
                            let _ = grid.map_pss(symbol, &pss_sequence, config.k_ssb);
                        }
                    }
                    
                    // Map SSS
                    if frame_structure.is_sss_symbol(symbol) {
                        let sss_symbols = if frame % 2 == 0 {
                            &sss_sequence_even
                        } else {
                            &sss_sequence_odd
                        };
                        {
                            let mut grid = resource_grid.lock().await;
                            let _ = grid.map_sss(symbol, sss_symbols, config.k_ssb);
                        }
                    }
                }
                
                // Map PBCH if needed
                let should_send_pbch = frame_structure.is_pbch_symbol(frame, slot, symbol);
                if should_send_pbch {
                    let ssb_idx = frame_structure.get_ssb_index(slot, symbol).unwrap_or(0);
                    let ssb_start = frame_structure.get_ssb_start_symbol(ssb_idx).unwrap_or(0);
                    let relative_symbol = symbol - ssb_start;
                    
                    let pbch_symbols = if relative_symbol == 1 {
                        let mib = pbch_processor.generate_mib(frame);
                        let symbols = pbch_processor.encode_pbch(&mib, frame);
                        state_guard.current_pbch_symbols.insert(ssb_idx, symbols.clone());
                        symbols
                    } else if relative_symbol == 3 {
                        state_guard.current_pbch_symbols.get(&ssb_idx).cloned()
                            .unwrap_or_else(|| {
                                let mib = pbch_processor.generate_mib(frame);
                                let symbols = pbch_processor.encode_pbch(&mib, frame);
                                state_guard.current_pbch_symbols.insert(ssb_idx, symbols.clone());
                                symbols
                            })
                    } else {
                        let mib = pbch_processor.generate_mib(frame);
                        pbch_processor.encode_pbch(&mib, frame)
                    };
                    
                    {
                        let mut grid = resource_grid.lock().await;
                        let _ = grid.map_pbch(relative_symbol, symbol, &pbch_symbols, config.k_ssb);
                        let _ = grid.map_pbch_dmrs(relative_symbol, symbol, config.cell_id.0, config.k_ssb, 
                                                   ssb_idx, frame);
                    }
                }
                
                // OFDM modulation
                let time_samples = {
                    let grid = resource_grid.lock().await;
                    let samples = ofdm_modulator.modulate(&*grid, symbol);
                    // No need to resize - OFDM modulator already produces correct number of samples
                    // based on FFT size and CP length
                    samples
                };
                
                // Create IQ buffer
                let timestamp = state_guard.sample_count;
                let iq_buffer = IqBuffer::from_samples(time_samples, timestamp, 0);
                
                // Send to RF channel
                let non_zero_count = iq_buffer.samples.iter().filter(|s| s.norm() > 0.0).count();
                if non_zero_count > 0 {
                    debug!("Pre-buffer: Generated {} non-zero samples at frame={}, slot={}, symbol={}", 
                          non_zero_count, frame, slot, symbol);
                }
                
                // Try to send but don't block - this fills the circular buffer
                let _ = rf_tx_channel.try_send(iq_buffer);
                
                // Update state with sample-based timing
                state_guard.sample_count += samples_per_symbol as u64;
                state_guard.expected_sample_count += samples_per_symbol as u64;
                state_guard.advance_symbol(symbols_per_slot, slots_per_frame);
                
                // Progress logging
                pre_buffer_count += 1;
                if pre_buffer_count % 100 == 0 {
                    info!("Pre-buffered {} symbols...", pre_buffer_count);
                }
                
                // Release lock for next iteration
                drop(state_guard);
            }
            
            info!("Pre-buffering complete! Generated {} symbols", pre_buffer_count);
            
            // Log timing alignment info
            let pre_buffered_frames = pre_buffer_count as f64 * symbol_duration.as_secs_f64() / 0.01; // frames = time / 10ms
            info!("Pre-buffered {:.1} frames - UE will start receiving from frame {:.0}", 
                  pre_buffered_frames, pre_buffered_frames);
            info!("Next SSB will be at frame {} (ensuring proper alignment)", 
                  ((pre_buffered_frames / 2.0).ceil() as u32) * 2);
            info!("Starting normal timed transmission...");
            
            // Initialize sample-based timing for normal operation
            // No more sleep-based timing - everything is sample-driven
            let samples_per_symbol_u64 = samples_per_symbol as u64;
            
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
                    
                    // Map synchronization signals (PSS/SSS) 
                    // Always use frame_structure for SSB timing since it knows all 4 SSB positions
                    // MAC only indicates if SSB should be sent in this slot, not individual SSB timing
                    let should_send_ssb = if let Some(schedule) = &slot_schedule {
                        if schedule.ssb_info.is_some() {
                            // MAC says SSB should be sent in this slot, use frame_structure for exact timing
                            frame_structure.is_sync_symbol(frame, slot, symbol)
                        } else {
                            false
                        }
                    } else {
                        // No MAC schedule, use frame_structure fallback
                        frame_structure.is_sync_symbol(frame, slot, symbol)
                    };
                    
                    // Log SSB transmission opportunity every 20ms (when we start a new SSB period)
                    if frame % 2 == 0 && slot == 0 && symbol == 2 {
                        let absolute_time_ms = (state_guard.sample_count as f64 / natural_sample_rate * 1000.0) as u64;
                        info!("SSB transmission period starting at frame={}, absolute_time={}ms, first SSB at slot 0 symbol 2", 
                              frame, absolute_time_ms);
                        info!("  Expected SSBs: slot 0 symbols 2-5,8-11 and slot 1 symbols 2-5,8-11");
                    }
                    
                    // Debug logging for SSB transmission decision at symbol 8
                    if frame % 2 == 0 && slot == 0 && symbol == 8 {
                        let sync_check = frame_structure.is_sync_symbol(frame, slot, symbol);
                        let pss_check = frame_structure.is_pss_symbol(symbol);
                        info!("DEBUG: Symbol 8 SSB check - frame={}, slot={}, symbol={}, is_sync_symbol={}, is_pss_symbol={}, should_send_ssb={}", 
                              frame, slot, symbol, sync_check, pss_check, should_send_ssb);
                        if let Some(schedule) = &slot_schedule {
                            if let Some(ssb_info) = &schedule.ssb_info {
                                let in_range = symbol >= ssb_info.start_symbol && symbol < ssb_info.start_symbol + 4;
                                info!("  MAC SSB info: start_symbol={}, spans symbols {}-{}, symbol {} in range: {}", 
                                      ssb_info.start_symbol, ssb_info.start_symbol, ssb_info.start_symbol + 3, symbol, in_range);
                            } else {
                                info!("  MAC SSB info: None");
                            }
                        } else {
                            info!("  No MAC schedule available");
                        }
                    }
                    
                    // Also debug slot 1 symbols
                    if frame % 2 == 0 && slot == 1 && (symbol == 2 || symbol == 8) {
                        info!("DEBUG: Slot 1 SSB check - frame={}, slot={}, symbol={}, should_send_ssb={}", 
                              frame, slot, symbol, should_send_ssb);
                    }
                    
                    if should_send_ssb {
                        // Get SSB index for debug logging
                        let ssb_index = frame_structure.get_ssb_index(slot, symbol);
                        
                        // OPTIMIZATION: Using pre-computed PSS sequence
                        if frame_structure.is_pss_symbol(symbol) {
                            if let Some(idx) = ssb_index {
                                let absolute_time_ms = (state_guard.sample_count as f64 / natural_sample_rate * 1000.0) as u64;
                                info!("Mapping PSS for SSB #{} at frame={}, slot={}, symbol={} (time={}ms)", 
                                      idx, frame, slot, symbol, absolute_time_ms);
                                
                                // Verify timing alignment
                                if frame == 0 && slot == 0 && symbol == 2 {
                                    info!("TIMING CHECK: First PSS transmitted at expected position (frame 0, slot 0, symbol 2)");
                                }
                            }
                            {
                                let mut grid = resource_grid.lock().await;
                                let _ = grid.map_pss(symbol, &pss_sequence, config.k_ssb);
                            }
                        }
                        
                        // OPTIMIZATION: Using pre-computed SSS sequences
                        if frame_structure.is_sss_symbol(symbol) {
                            debug!("Mapping SSS to resource grid at frame={}, slot={}, symbol={}", frame, slot, symbol);
                            let sss_symbols = if frame % 2 == 0 {
                                &sss_sequence_even
                            } else {
                                &sss_sequence_odd
                            };
                            {
                                let mut grid = resource_grid.lock().await;
                                let _ = grid.map_sss(symbol, sss_symbols, config.k_ssb);
                            }
                        }
                    }
                    
                    // Map PBCH based on MAC scheduling or fallback
                    let should_send_pbch = slot_schedule.as_ref()
                        .and_then(|s| s.ssb_info.as_ref())
                        .map(|_ssb| frame_structure.is_pbch_symbol(frame, slot, symbol))
                        .unwrap_or_else(|| frame_structure.is_pbch_symbol(frame, slot, symbol));
                    
                    // Debug logging for PBCH - check all PBCH symbols
                    if frame_structure.is_sync_slot(frame, slot) && frame_structure.is_pbch_symbol(frame, slot, symbol) {
                        debug!("PBCH check: frame={}, slot={}, symbol={}, should_send_pbch={}", 
                               frame, slot, symbol, should_send_pbch);
                    }
                        
                    if should_send_pbch {
                        info!("Mapping PBCH to resource grid at frame={}, slot={}, symbol={}", frame, slot, symbol);
                        
                        // Get SSB index for this slot/symbol
                        let ssb_idx = frame_structure.get_ssb_index(slot, symbol).unwrap_or(0);
                        
                        // Generate PBCH symbols once per SSB block (cache for reuse)
                        // We need to generate at the start of each SSB block
                        let ssb_start = frame_structure.get_ssb_start_symbol(ssb_idx).unwrap_or(0);
                        let relative_symbol = symbol - ssb_start;
                        
                        info!("PBCH mapping debug: slot={}, symbol={}, ssb_idx={}, ssb_start={}, relative_symbol={}",
                              slot, symbol, ssb_idx, ssb_start, relative_symbol);
                        
                        // Generate PBCH only at the start of SSB block (when relative_symbol == 1)
                        // For other PBCH positions, reuse the same symbols
                        let pbch_symbols = if relative_symbol == 1 {
                            // First PBCH symbol in SSB - generate new PBCH data
                            info!("Generating new PBCH symbols for SSB #{}", ssb_idx);
                            let mib = pbch_processor.generate_mib(frame);
                            let symbols = pbch_processor.encode_pbch(&mib, frame);
                            info!("Generated {} PBCH symbols, storing in cache with key {}", symbols.len(), ssb_idx);
                            // Store for reuse in symbol 3, indexed by SSB index
                            state_guard.current_pbch_symbols.insert(ssb_idx, symbols.clone());
                            info!("Cache now contains {} entries", state_guard.current_pbch_symbols.len());
                            symbols
                        } else if relative_symbol == 3 {
                            // Second PBCH symbol in SSB - reuse cached symbols for this SSB index
                            info!("Looking for cached PBCH symbols for SSB #{}", ssb_idx);
                            info!("Cache contains {} entries, keys: {:?}", 
                                  state_guard.current_pbch_symbols.len(),
                                  state_guard.current_pbch_symbols.keys().collect::<Vec<_>>());
                            
                            state_guard.current_pbch_symbols.get(&ssb_idx).cloned()
                                .map(|symbols| {
                                    info!("Found cached PBCH symbols for SSB #{}: {} symbols", ssb_idx, symbols.len());
                                    symbols
                                })
                                .unwrap_or_else(|| {
                                    // Fallback: generate if not cached
                                    warn!("PBCH symbols not found in cache for SSB #{}, generating new", ssb_idx);
                                    let mib = pbch_processor.generate_mib(frame);
                                    let symbols = pbch_processor.encode_pbch(&mib, frame);
                                    // Also cache for future use
                                    state_guard.current_pbch_symbols.insert(ssb_idx, symbols.clone());
                                    symbols
                                })
                        } else {
                            // This shouldn't happen for valid PBCH symbols
                            warn!("Unexpected PBCH symbol position: relative_symbol={}", relative_symbol);
                            let mib = pbch_processor.generate_mib(frame);
                            pbch_processor.encode_pbch(&mib, frame)
                        };
                        
                        // Check PBCH symbols before mapping
                        let pbch_power: f32 = pbch_symbols.iter().map(|s| s.norm_sqr()).sum::<f32>() / pbch_symbols.len() as f32;
                        let pbch_power_db = 10.0 * pbch_power.log10();
                        info!("PBCH symbols power before mapping: {:.2} dB ({} symbols)", pbch_power_db, pbch_symbols.len());
                        
                        {
                            let mut grid = resource_grid.lock().await;
                            // Map PBCH data symbols using relative symbol position and actual grid symbol
                            // The map_pbch function expects relative position within SSB (1, 2, or 3) and actual symbol
                            let _ = grid.map_pbch(relative_symbol, symbol, &pbch_symbols, config.k_ssb);
                            // Map PBCH DMRS with proper parameters for Gold sequence
                            let _ = grid.map_pbch_dmrs(relative_symbol, symbol, config.cell_id.0, config.k_ssb, 
                                                       ssb_idx, frame);
                        }
                        info!("PBCH with DMRS mapped for relative_symbol={}", relative_symbol);
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
                    
                    // CRITICAL: Always transmit samples to maintain continuous ZMQ flow
                    // The UE expects continuous sample stream for proper cell detection
                    
                    // OFDM modulation - ALWAYS generate samples for continuous transmission
                    let time_samples = {
                        let grid = resource_grid.lock().await;
                        let mut samples = ofdm_modulator.modulate(&*grid, symbol);
                        // Ensure correct number of samples
                        samples.resize(samples_per_symbol, num_complex::Complex32::new(0.0, 0.0));
                        
                        // Check if we have actual signal content
                        let has_signal = samples.iter().any(|s| s.norm_sqr() > 1e-10);
                        
                        // Enhanced IQ sample logging for SSB analysis
                        if should_send_ssb {
                            let ssb_idx = frame_structure.get_ssb_index(slot, symbol).unwrap_or(99);
                            
                            // Calculate signal statistics
                            let power: f32 = samples.iter().map(|s| s.norm_sqr()).sum::<f32>() / samples.len() as f32;
                            let power_db = 10.0 * power.log10();
                            let peak_sample = samples.iter().map(|s| s.norm()).max_by(|a, b| a.partial_cmp(b).unwrap()).unwrap_or(0.0);
                            let peak_db = 20.0 * peak_sample.log10();
                            let rms = power.sqrt();
                            
                            // Check for signal anomalies
                            let mut zero_count = 0;
                            let mut nan_count = 0;
                            let mut inf_count = 0;
                            let mut saturated_count = 0;
                            
                            for sample in samples.iter() {
                                if sample.re == 0.0 && sample.im == 0.0 {
                                    zero_count += 1;
                                } else if sample.re.is_nan() || sample.im.is_nan() {
                                    nan_count += 1;
                                } else if sample.re.is_infinite() || sample.im.is_infinite() {
                                    inf_count += 1;
                                } else if sample.norm() > 0.9 {
                                    saturated_count += 1;
                                }
                            }
                            
                            // Detailed logging for PSS
                            if frame_structure.is_pss_symbol(symbol) {
                                info!("=== PSS IQ Sample Analysis for SSB #{} ===", ssb_idx);
                                info!("Frame: {}, Slot: {}, Symbol: {}", frame, slot, symbol);
                                info!("Signal Statistics:");
                                info!("  Average Power: {:.2} dB ({:.6} linear)", power_db, power);
                                info!("  Peak Amplitude: {:.2} dB ({:.6} linear)", peak_db, peak_sample);
                                info!("  RMS: {:.6}", rms);
                                info!("  Total Samples: {}", samples.len());
                                
                                // Signal quality analysis
                                info!("Signal Quality:");
                                info!("  Zero samples: {} ({:.1}%)", zero_count, 100.0 * zero_count as f32 / samples.len() as f32);
                                info!("  NaN samples: {}", nan_count);
                                info!("  Inf samples: {}", inf_count);
                                info!("  Saturated samples (>0.9): {} ({:.1}%)", saturated_count, 100.0 * saturated_count as f32 / samples.len() as f32);
                                
                                // Log first 100 IQ samples
                                info!("First 100 IQ samples after OFDM modulation:");
                                for (i, sample) in samples.iter().take(100).enumerate() {
                                    if i % 10 == 0 {
                                        info!("  Samples [{:02}-{:02}]:", i, (i+9).min(99));
                                    }
                                    info!("    [{:02}] = {:+.6} {:+.6}j (mag: {:.6}, phase: {:+.3}°)", 
                                          i, sample.re, sample.im, sample.norm(), sample.arg() * 180.0 / std::f32::consts::PI);
                                }
                                
                                // Warnings
                                if nan_count > 0 {
                                    error!("ERROR: Found {} NaN samples in PSS!", nan_count);
                                }
                                if inf_count > 0 {
                                    error!("ERROR: Found {} Inf samples in PSS!", inf_count);
                                }
                                if peak_sample > 0.9 {
                                    warn!("WARNING: Signal may be saturated! Peak amplitude: {:.3}", peak_sample);
                                }
                                if power_db < -40.0 {
                                    warn!("WARNING: Signal power very low: {:.2} dB", power_db);
                                }
                                if power_db > 0.0 {
                                    warn!("WARNING: Signal power may be too high: {:.2} dB", power_db);
                                }
                                
                                info!("=== End PSS IQ Sample Analysis ===");
                            }
                            
                            // Brief logging for SSS
                            if frame_structure.is_sss_symbol(symbol) {
                                info!("SSS signal power for SSB #{}: avg={:.2} dB, peak={:.2} dB, RMS={:.4}", 
                                      ssb_idx, power_db, peak_db, rms);
                            }
                            
                            // Brief logging for PBCH
                            if frame_structure.is_pbch_symbol(frame, slot, symbol) {
                                info!("PBCH signal power at symbol {}: avg={:.2} dB, peak={:.2} dB", 
                                      symbol, power_db, peak_db);
                            }
                        }
                        
                        // If no signal content, generate low-level noise to maintain timing
                        // This ensures the UE's ZMQ receiver doesn't stall
                        // Match srsRAN: No artificial noise generation
                        // Empty symbols remain as zeros after IFFT
                        // This allows the signal to be properly detected
                        
                        samples
                    };
                    
                    // Apply resampling if configured (convert from PHY rate to RF rate)
                    let resampled_samples = if let Some(ref resampler_arc) = resampler {
                        let mut resampler_guard = resampler_arc.lock().await;
                        resampler_guard.process(&time_samples)
                    } else {
                        time_samples
                    };
                    
                    // Create IQ buffer with proper timestamp
                    // Use the sample count at the START of this symbol for proper alignment
                    let timestamp = state_guard.sample_count - samples_per_symbol as u64;  // Before we updated it
                    let iq_buffer = IqBuffer::from_samples(resampled_samples, timestamp, 0);
                    
                    // Debug: Track what we're sending
                    let non_zero_count = iq_buffer.samples.iter().filter(|s| s.norm() > 0.0).count();
                    if non_zero_count > 0 {
                        info!("PHY DL: Sending {} non-zero samples at frame={}, slot={}, symbol={}",
                              non_zero_count, frame, slot, symbol);
                    }
                    
                    // Send samples to RF channel - try to send but don't block forever
                    // Use try_send to avoid blocking when channel is full
                    
                    // DEBUG: Calculate signal power before sending to RF
                    if non_zero_count > 0 {
                        let avg_power: f32 = iq_buffer.samples.iter().map(|s| s.norm_sqr()).sum::<f32>() / iq_buffer.samples.len() as f32;
                        let peak_amplitude = iq_buffer.samples.iter().map(|s| s.norm()).fold(0.0_f32, f32::max);
                        let avg_power_db = 10.0 * avg_power.log10();
                        let peak_db = 20.0 * peak_amplitude.log10();
                        
                        info!("PHY->RF HANDOFF: Sending {} samples with {} non-zero", iq_buffer.samples.len(), non_zero_count);
                        info!("  Average power: {:.3} ({:.1} dB)", avg_power, avg_power_db);
                        info!("  Peak amplitude: {:.3} ({:.1} dB)", peak_amplitude, peak_db);
                        info!("  Frame={}, Slot={}, Symbol={}", frame, slot, symbol);
                    }
                    
                    match rf_tx_channel.try_send(iq_buffer) {
                        Ok(_) => {
                            if non_zero_count > 0 {
                                info!("PHY DL: Successfully queued samples to RF channel");
                            }
                        }
                        Err(tokio::sync::mpsc::error::TrySendError::Full(_)) => {
                            // Channel is full - this means RF is not consuming fast enough
                            // Drop this buffer to maintain timing
                            static mut FULL_COUNT: u64 = 0;
                            unsafe {
                                FULL_COUNT += 1;
                                if FULL_COUNT % 100 == 1 {
                                    warn!("PHY DL: RF channel full, dropped {} buffers total", FULL_COUNT);
                                }
                            }
                        }
                        Err(tokio::sync::mpsc::error::TrySendError::Closed(_)) => {
                            error!("PHY DL: RF channel closed, stopping downlink processing");
                            break;
                        }
                    }
                    
                    // Update state with sample-based timing
                    let previous_sample_count = state_guard.sample_count;
                    state_guard.sample_count += samples_per_symbol as u64;
                    state_guard.expected_sample_count += samples_per_symbol_u64;
                    
                    // Check timing alignment
                    let sample_drift = state_guard.sample_count as i64 - state_guard.expected_sample_count as i64;
                    if sample_drift.abs() > samples_per_symbol_u64 as i64 {
                        warn!("Sample timing drift detected: {} samples (expected: {}, actual: {})",
                              sample_drift, state_guard.expected_sample_count, state_guard.sample_count);
                        // Realign by adjusting expected count
                        state_guard.expected_sample_count = state_guard.sample_count;
                    }
                    
                    // Advance symbol/slot/frame
                    state_guard.advance_symbol(symbols_per_slot, slots_per_frame);
                    
                    // Drop the state guard to avoid holding the lock
                    drop(state_guard);
                    
                    // Sample-based timing: No sleep!
                    // The ZMQ interface will control the pacing based on sample consumption
                    // This ensures perfect synchronization with the UE's expected sample rate
                    
                    // Small yield to prevent CPU spinning in tight loop
                    tokio::task::yield_now().await;
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
            
            // Sample-based timing for uplink - no more sleep!
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
                
                // Sample-based timing: No sleep!
                // The uplink processing is driven by received samples from ZMQ
                // Just yield to prevent CPU spinning
                tokio::task::yield_now().await;
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
pub fn calculate_fft_size(bandwidth: &Bandwidth, scs: &SubcarrierSpacing) -> Result<usize, LayerError> {
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
}pub mod ofdm_backend;
#[cfg(feature = "flexran")]
pub mod flexran_adapter;
