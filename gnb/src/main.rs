//! Albor Space 5G GNodeB Main Application
//! 
//! This is the main entry point for the 5G base station implementation.

use anyhow::Result;
use clap::Parser;
use tracing::{info, error, warn};
use tracing_subscriber::{EnvFilter, fmt};
use std::sync::Arc;
use tokio::sync::RwLock;

use common::types::{Pci, CellId, Bandwidth, SubcarrierSpacing};
use interfaces::zmq_rf::ZmqRfConfig;
use layers::phy::{EnhancedPhyLayer, PhyConfig, CyclicPrefix, DuplexMode};
use layers::mac::{EnhancedMacLayer, MacConfig, default_sib1_config};
use layers::rrc::{RrcLayer, RrcConfig, RrcMacInterface};
use layers::ngap::{NgapLayer, NgapConfig};
use layers::ProtocolLayer;
use std::net::SocketAddr;
use std::str::FromStr;

mod config;
use config::GnbConfig;

/// Albor Space 5G GNodeB
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to YAML configuration file
    #[arg(short, long)]
    config: String,

    /// Log level override (trace, debug, info, warn, error)
    #[arg(short, long)]
    log_level: Option<String>,
}


/// GNodeB application state
struct GnbState {
    phy_layer: Arc<RwLock<EnhancedPhyLayer>>,
    mac_layer: Arc<EnhancedMacLayer>,
    rrc_layer: Arc<RwLock<RrcLayer>>,
    ngap_layer: Arc<RwLock<NgapLayer>>,
    running: Arc<RwLock<bool>>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Load YAML configuration
    info!("Loading configuration from: {}", args.config);
    let config = GnbConfig::from_yaml_file(&args.config)?;
    
    // Initialize logging with level from config or override
    let log_level = args.log_level.as_ref()
        .unwrap_or(&config.log.all_level);
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(log_level));
    
    fmt()
        .with_env_filter(env_filter)
        .with_target(true)
        .with_thread_ids(true)
        .with_line_number(true)
        .init();

    info!("Starting Albor Space 5G GNodeB");
    info!("Configuration loaded from: {}", args.config);
    
    // Extract and validate configuration parameters
    let pci = Pci::new(config.cell_cfg.pci)
        .ok_or_else(|| anyhow::anyhow!("Invalid PCI: {}", config.cell_cfg.pci))?;
    
    let cell_id = CellId(config.cell_cfg.pci); // Using PCI as cell ID for now
    
    let bandwidth = match config.cell_cfg.channel_bandwidth_mhz {
        5 => Bandwidth::Bw5,
        10 => Bandwidth::Bw10,
        15 => Bandwidth::Bw15,
        20 => Bandwidth::Bw20,
        25 => Bandwidth::Bw25,
        30 => Bandwidth::Bw30,
        40 => Bandwidth::Bw40,
        50 => Bandwidth::Bw50,
        60 => Bandwidth::Bw60,
        80 => Bandwidth::Bw80,
        100 => Bandwidth::Bw100,
        _ => return Err(anyhow::anyhow!("Invalid bandwidth: {} MHz", config.cell_cfg.channel_bandwidth_mhz)),
    };
    
    let scs = match config.cell_cfg.common_scs {
        15 => SubcarrierSpacing::Scs15,
        30 => SubcarrierSpacing::Scs30,
        60 => SubcarrierSpacing::Scs60,
        120 => SubcarrierSpacing::Scs120,
        240 => SubcarrierSpacing::Scs240,
        _ => return Err(anyhow::anyhow!("Invalid subcarrier spacing: {} kHz", config.cell_cfg.common_scs)),
    };
    
    // Calculate carrier frequency from ARFCN (Band 3 specific)
    let carrier_frequency = calculate_frequency_from_arfcn(config.cell_cfg.dl_arfcn, config.cell_cfg.band)?;
    
    // For Band 3 with 10 MHz bandwidth, the SSB is placed 450 kHz below carrier center
    // This corresponds to 30 subcarriers with 15 kHz SCS
    // SSB ARFCN 368410 vs DL ARFCN 368500 = 450 kHz offset
    let k_ssb = -30i16;  // Negative because SSB is below carrier center
    
    info!("SSB configuration:");
    info!("  Carrier frequency: {} MHz", carrier_frequency / 1e6);
    info!("  SSB placement: 450 kHz below carrier center");
    info!("  k_SSB: {} (30 subcarriers below carrier)", k_ssb);
    
    info!("Cell configuration:");
    info!("  PCI: {}", pci.0);
    info!("  Cell ID: {}", cell_id.0);
    info!("  Band: {}", config.cell_cfg.band);
    info!("  DL ARFCN: {}", config.cell_cfg.dl_arfcn);
    info!("  Carrier frequency: {} MHz", carrier_frequency / 1e6);
    info!("  Bandwidth: {} MHz", config.cell_cfg.channel_bandwidth_mhz);
    info!("  Subcarrier spacing: {} kHz", config.cell_cfg.common_scs);
    info!("  PLMN: {}", config.cell_cfg.plmn);
    info!("  TAC: {}", config.cell_cfg.tac);
    info!("  PHY mode: Enhanced (full)");
    
    // AMF configuration
    info!("AMF configuration:");
    info!("  Address: {}:{}", config.cu_cp.amf.addr, config.cu_cp.amf.port);
    info!("  Bind address: {}", config.cu_cp.amf.bind_addr);
    
    // Parse PLMN from config (format: "00101" -> [0x00, 0xF1, 0x10])
    let plmn_str = &config.cell_cfg.plmn;
    if plmn_str.len() != 5 && plmn_str.len() != 6 {
        return Err(anyhow::anyhow!("Invalid PLMN format: {}", plmn_str));
    }
    
    let mcc = &plmn_str[0..3];
    let mnc = &plmn_str[3..];
    
    // Convert to BCD format for NGAP
    let mut plmn_id = [0u8; 3];
    plmn_id[0] = ((mcc.chars().nth(1).unwrap().to_digit(10).unwrap() as u8) << 4) | 
                  (mcc.chars().nth(0).unwrap().to_digit(10).unwrap() as u8);
    plmn_id[1] = ((mnc.chars().nth(0).unwrap().to_digit(10).unwrap() as u8) << 4) | 
                  (mcc.chars().nth(2).unwrap().to_digit(10).unwrap() as u8);
    if mnc.len() == 2 {
        plmn_id[2] = 0xF0 | (mnc.chars().nth(1).unwrap().to_digit(10).unwrap() as u8);
    } else {
        plmn_id[2] = ((mnc.chars().nth(2).unwrap().to_digit(10).unwrap() as u8) << 4) | 
                      (mnc.chars().nth(1).unwrap().to_digit(10).unwrap() as u8);
    }
    
    // Create PRACH configuration from config file
    let prach_config = layers::phy::prach::RachConfigCommon {
        prach_config_index: config.cell_cfg.prach.prach_config_index,
        ra_response_window: 10,  // Default value
        msg1_fdm: 1,  // Default value
        msg1_frequency_start: config.cell_cfg.prach.prach_frequency_start as u32,
        zero_correlation_zone_config: config.cell_cfg.prach.zero_correlation_zone as u16,
        preamble_rx_target_power: -104,  // Default value
        preamble_trans_max: 7,  // Default value
        power_ramping_step_db: 4,  // Default value
        total_num_ra_preambles: 64,  // Default value
        prach_root_seq_index: config.cell_cfg.prach.prach_root_sequence_index,
        msg1_scs: layers::phy::prach::PrachSubcarrierSpacing::Khz1_25,  // Default for long preambles
        restricted_set: layers::phy::prach::RestrictedSetConfig::UnrestrictedSet,
    };

    // Create PHY configuration
    let phy_config = PhyConfig {
        pci,
        cell_id,
        carrier_frequency,
        bandwidth,
        subcarrier_spacing: scs,
        num_tx_antennas: 1,
        num_rx_antennas: 1,
        cyclic_prefix: CyclicPrefix::Normal,
        duplex_mode: DuplexMode::Fdd,  // Band 3 is FDD
        k_ssb,
        sample_rate: config.ru_sdr.srate * 1e6,  // Convert from MHz to Hz
        prach_config,
    };
    
    // Create ZMQ RF configuration from device args in config
    let mut zmq_config = ZmqRfConfig::from_device_args(&config.ru_sdr.device_args, 1)?;
    zmq_config.tx_gain = config.ru_sdr.tx_gain;
    zmq_config.rx_gain = config.ru_sdr.rx_gain;
    
    // Override sample rate with natural FFT-based sample rate
    // This ensures perfect alignment with OFDM processing
    let fft_size = layers::phy::calculate_fft_size(&bandwidth, &scs)?;
    let scs_hz = match scs {
        SubcarrierSpacing::Scs15 => 15_000.0,
        SubcarrierSpacing::Scs30 => 30_000.0,
        SubcarrierSpacing::Scs60 => 60_000.0,
        SubcarrierSpacing::Scs120 => 120_000.0,
        SubcarrierSpacing::Scs240 => 240_000.0,
    };
    let natural_sample_rate = fft_size as f64 * scs_hz;
    // REMOVED: Sample rate override that was preventing resampler from working
    // zmq_config.sample_rate = natural_sample_rate;
    
    info!("ZMQ configuration:");
    info!("  TX address: {}", zmq_config.tx_address);
    info!("  RX address: {}", zmq_config.rx_address);
    info!("  Sample rate: {} MHz (from config)", zmq_config.sample_rate / 1e6);
    info!("  PHY natural rate: {} MHz (FFT size {} × SCS {} kHz)", 
          natural_sample_rate / 1e6, fft_size, scs_hz / 1000.0);

    // Create MAC configuration
    let mac_config = MacConfig {
        cell_id,
        scs,
        bandwidth,
        max_ues: 32,
        sib1_config: default_sib1_config(cell_id),
        coreset0_index: config.cell_cfg.pdcch.common.coreset0_index,
    };
    
    // Initialize MAC layer
    let mut mac_layer = EnhancedMacLayer::new(mac_config.clone())?;
    
    // Create RRC configuration
    let rrc_config = RrcConfig {
        sib_periodicity: 160,
        max_ue_contexts: 100,
        cell_id,
        plmn_id,
        tac: config.cell_cfg.tac,
    };
    
    // Initialize RRC layer
    let mut rrc_layer = RrcLayer::new(rrc_config);
    
    // Create channels between MAC and RRC
    use bytes::Bytes;
    let (mac_to_rrc_tx, mut mac_to_rrc_rx) = tokio::sync::mpsc::channel::<(common::types::Rnti, Bytes)>(100);
    let (rrc_to_mac_tx, mut rrc_to_mac_rx) = tokio::sync::mpsc::channel::<(common::types::Rnti, layers::rrc::RrcMessageType, Bytes)>(100);
    
    // Set channel before creating Arc
    mac_layer.set_rrc_channel(mac_to_rrc_tx);
    
    // Now initialize and create Arc
    mac_layer.initialize().await?;
    info!("MAC layer initialized");
    let mac_layer = Arc::new(mac_layer);
    
    // Set MAC interface for RRC
    let mac_interface: Arc<dyn RrcMacInterface> = mac_layer.clone() as Arc<dyn RrcMacInterface>;
    rrc_layer.set_mac_interface(mac_interface);
    
    rrc_layer.initialize().await
        .map_err(|e| anyhow::anyhow!("Failed to initialize RRC layer: {}", e))?;
    info!("RRC layer initialized");
    let rrc_layer = Arc::new(RwLock::new(rrc_layer));
    
    // Initialize PHY layer
    let mut enhanced_phy = EnhancedPhyLayer::new(phy_config)?;
    
    // Set MAC interface for PHY - cast to trait object
    let mac_interface: Arc<dyn layers::mac::MacPhyInterface> = mac_layer.clone() as Arc<dyn layers::mac::MacPhyInterface>;
    enhanced_phy.set_mac_interface(mac_interface);
    
    enhanced_phy.initialize_with_rf(zmq_config).await?;
    info!("Enhanced PHY layer initialized (full mode)");
    let phy_layer = Arc::new(RwLock::new(enhanced_phy));
    
    // Create NGAP configuration
    let amf_addr = format!("{}:{}", config.cu_cp.amf.addr, config.cu_cp.amf.port);
    // Use AMF address from configuration
    let ngap_config = NgapConfig {
        amf_address: SocketAddr::from_str(&amf_addr)
            .map_err(|e| anyhow::anyhow!("Invalid AMF address {}: {}", amf_addr, e))?,
        local_address: SocketAddr::from_str(&format!("{}:0", config.cu_cp.amf.bind_addr))
            .map_err(|e| anyhow::anyhow!("Invalid bind address: {}", e))?,
        gnb_id: config.cell_cfg.pci as u32, // Using PCI as gNB ID for now
        plmn_id,
    };
    
    // Initialize NGAP layer
    let mut ngap_layer = NgapLayer::new(ngap_config);
    match ngap_layer.initialize().await {
        Ok(_) => info!("NGAP layer initialized and connected to AMF"),
        Err(e) => {
            warn!("Failed to initialize NGAP layer: {}. Continuing without AMF connection.", e);
            warn!("Cell will broadcast but no core network connectivity.");
        }
    }
    let ngap_layer = Arc::new(RwLock::new(ngap_layer));
    
    let running = Arc::new(RwLock::new(true));
    
    let state = GnbState {
        phy_layer,
        mac_layer,
        rrc_layer,
        ngap_layer,
        running: running.clone(),
    };

    info!("GNodeB initialized successfully");
    
    // Start PHY processing in background
    let phy_handle = {
        let phy = state.phy_layer.clone();
        tokio::spawn(async move {
            let phy_guard = phy.read().await;
            if let Err(e) = phy_guard.start_processing().await {
                error!("Enhanced PHY processing error: {}", e);
            }
        })
    };
    
    // Start RRC message processing task
    let rrc_handle = {
        let rrc = state.rrc_layer.clone();
        let running = running.clone();
        tokio::spawn(async move {
            while *running.read().await {
                // Process messages from MAC
                if let Some((rnti, data)) = mac_to_rrc_rx.recv().await {
                    let mut rrc_guard = rrc.write().await;
                    if let Err(e) = rrc_guard.process_uplink(data).await {
                        error!("RRC uplink processing error: {}", e);
                    }
                }
            }
        })
    };
    
    // Start statistics reporting
    let stats_handle = {
        let phy = state.phy_layer.clone();
        let running = running.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(5));
            
            while *running.read().await {
                interval.tick().await;
                
                let phy_guard = phy.read().await;
                let stats = phy_guard.get_stats().await;
                
                info!("PHY Statistics:");
                info!("  Frame: {}, Slot: {}, Symbol: {}", 
                      stats.frame_number, stats.slot_number, stats.symbol_number);
                info!("  Samples processed: {}", stats.sample_count);
                
                if let Some(rf_stats) = stats.rf_stats {
                    info!("  TX samples: {}, RX samples: {}", 
                          rf_stats.tx_samples, rf_stats.rx_samples);
                    info!("  TX underruns: {}, RX overruns: {}", 
                          rf_stats.tx_underruns, rf_stats.rx_overruns);
                }
            }
        })
    };

    // Wait for shutdown signal
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {
            info!("Received shutdown signal");
        }
        _ = phy_handle => {
            warn!("PHY processing stopped unexpectedly");
        }
        _ = rrc_handle => {
            warn!("RRC processing stopped unexpectedly");
        }
    }
    
    // Shutdown
    info!("Shutting down GNodeB");
    *running.write().await = false;
    
    // Stop PHY processing
    {
        let phy_guard = state.phy_layer.read().await;
        if let Err(e) = phy_guard.stop_processing().await {
            error!("Error stopping enhanced PHY: {}", e);
        }
    }
    
    // Shutdown RRC layer
    {
        let mut rrc_guard = state.rrc_layer.write().await;
        if let Err(e) = rrc_guard.shutdown().await {
            error!("Error shutting down RRC: {}", e);
        }
    }
    
    // Shutdown NGAP layer
    {
        let mut ngap_guard = state.ngap_layer.write().await;
        if let Err(e) = ngap_guard.shutdown().await {
            error!("Error shutting down NGAP: {}", e);
        }
    }
    
    // Wait for tasks to complete
    let _ = tokio::time::timeout(
        tokio::time::Duration::from_secs(5),
        stats_handle
    ).await;
    
    info!("GNodeB shutdown complete");
    Ok(())
}

/// Calculate carrier frequency from ARFCN
fn calculate_frequency_from_arfcn(arfcn: u32, band: u16) -> Result<f64> {
    // For NR Band n3 (1800 MHz FDD)
    // DL: 1805-1880 MHz
    // NR-ARFCN range: 361000-376000
    // Formula: F_REF = F_REF-Offs + ΔF_Global * (N_REF - N_REF-Offs)
    // For Band n3: F_REF-Offs = 1805 MHz, N_REF-Offs = 361000, ΔF_Global = 5 kHz
    match band {
        3 => {
            if arfcn < 361000 || arfcn > 376000 {
                return Err(anyhow::anyhow!("Invalid NR-ARFCN {} for band n3", arfcn));
            }
            // NR frequency calculation for band n3
            Ok(1805.0e6 + 5.0e3 * (arfcn - 361000) as f64)
        }
        _ => Err(anyhow::anyhow!("Unsupported band: {}", band))
    }
}

/// Calculate SSB ARFCN from DL ARFCN and bandwidth for given band
/// According to 3GPP TS 38.104 and srsRAN implementation
/// Calculate GSCN (Global Synchronization Channel Number) for Band 3
fn calculate_gscn_for_band3(carrier_freq_mhz: f64) -> Result<u32> {
    // For Band 3 (1710-1880 MHz), GSCN range is 4517-4693
    // For Band 3, we should place SSB near the carrier center for compatibility
    // srsRAN typically uses carrier center for SSB in Band 3
    
    // Find closest GSCN to carrier frequency
    // GSCN = 3*N + (M-3)/2 where SS_ref = 1200*N + 50*M kHz
    // For Band 3: N = 1423, M must be odd
    
    let n = 1423;
    let base_freq_khz = 1200.0 * n as f64; // 1,707,600 kHz
    let target_freq_khz = carrier_freq_mhz * 1000.0;
    let offset_khz = target_freq_khz - base_freq_khz;
    
    // Calculate M to get closest to carrier frequency
    let m_float = offset_khz / 50.0;
    let m = if m_float.round() as i32 % 2 == 0 {
        // M must be odd, round to nearest odd
        (m_float.round() as i32 + 1).max(3)
    } else {
        m_float.round() as i32
    };
    
    // Ensure M is valid (odd and >= 3)
    let m = m.max(3);
    
    let gscn = 3 * n + (m - 3) / 2;
    let ss_ref_mhz = (base_freq_khz + 50.0 * m as f64) / 1000.0;
    
    // Verify GSCN is in Band 3 range
    if gscn < 4517 || gscn > 4693 {
        warn!("GSCN {} is outside Band 3 range (4517-4693), using closest valid", gscn);
        // For 1842.5 MHz, use GSCN 4625 (SS_ref = 1842.45 MHz)
        return Ok(4625);
    }
    
    info!("GSCN calculation: carrier={:.3} MHz, N={}, M={}, GSCN={}, SS_ref={:.3} MHz",
          carrier_freq_mhz, n, m, gscn, ss_ref_mhz);
    
    Ok(gscn as u32)
}

/// Calculate SSB center frequency from GSCN
fn calculate_ssb_freq_from_gscn(gscn: u32) -> f64 {
    // For GSCN range 4517-4693 (Band 3)
    if gscn < 4517 || gscn > 4693 {
        warn!("GSCN {} is outside Band 3 range (4517-4693)", gscn);
    }
    
    // For this range: GSCN = 3*N + (M-3)/2 where N=1423
    let n = 1423;
    let m = 2 * (gscn - 3 * n) + 3;
    
    // SS_ref in MHz
    let ss_ref_mhz = (1200.0 * n as f64 + 50.0 * m as f64) / 1000.0;
    
    ss_ref_mhz
}

/// Calculate Point A frequency for NR carrier
fn calculate_point_a(carrier_freq_hz: f64, n_rbs: u16, scs_khz: u32) -> f64 {
    // Point A is the lowest frequency of the carrier
    // Point A = carrier_center - (N_RB * 12 * SCS) / 2
    let bandwidth_hz = (n_rbs as f64) * 12.0 * (scs_khz as f64) * 1000.0;
    let point_a_hz = carrier_freq_hz - (bandwidth_hz / 2.0);
    
    info!("Point A calculation: carrier={:.3} MHz, N_RB={}, SCS={} kHz, BW={:.3} MHz, Point A={:.3} MHz",
          carrier_freq_hz / 1e6, n_rbs, scs_khz, bandwidth_hz / 1e6, point_a_hz / 1e6);
    
    point_a_hz
}

/// Calculate k_SSB (SSB subcarrier offset) from Point A to first SSB subcarrier
/// Per 3GPP TS 38.211, k_SSB is the offset from Point A to the first subcarrier of SSB
fn calculate_k_ssb(point_a_hz: f64, ssb_first_sc_hz: f64, scs_khz: u32) -> i16 {
    // Calculate frequency difference from Point A to first SSB subcarrier
    let freq_diff_hz = ssb_first_sc_hz - point_a_hz;
    
    // Convert to subcarriers (each subcarrier is scs_khz kHz wide)
    let k_ssb = (freq_diff_hz / (scs_khz as f64 * 1000.0)).round() as i16;
    
    info!("k_SSB calculation: Point A={:.3} MHz, SSB first SC={:.3} MHz, diff={:.3} MHz, SCS={} kHz, k_SSB={} subcarriers",
          point_a_hz / 1e6, ssb_first_sc_hz / 1e6, freq_diff_hz / 1e6, scs_khz, k_ssb);
    
    k_ssb
}