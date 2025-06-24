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
use layers::ProtocolLayer;

/// Albor Space 5G GNodeB
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to configuration file
    #[arg(short, long, default_value = "config.toml")]
    config: String,

    /// Log level (trace, debug, info, warn, error)
    #[arg(short, long, default_value = "info")]
    log_level: String,
    
    /// Physical Cell ID (0-1007)
    #[arg(long, default_value = "0")]
    pci: u16,
    
    /// Cell ID
    #[arg(long, default_value = "1")]
    cell_id: u16,
    
    /// Carrier frequency in MHz
    #[arg(long, default_value = "1842.5")]  // Band 3 FDD, DL ARFCN 368500
    frequency_mhz: f64,
    
    /// Bandwidth in MHz (5, 10, 15, 20, 25, 30, 40, 50, 60, 80, 100)
    #[arg(long, default_value = "10")]
    bandwidth_mhz: u32,
    
    /// Subcarrier spacing in kHz (15, 30, 60, 120, 240)
    #[arg(long, default_value = "15")]  // 15 kHz for FDD band 3
    scs_khz: u32,
    
    /// ZMQ device arguments (e.g., "tx_port=tcp://*:2000,rx_port=tcp://localhost:2001,base_srate=23.04e6")
    #[arg(long, default_value = "tx_port=tcp://*:2000,rx_port=tcp://localhost:2001,base_srate=23.04e6")]  // 23.04 MHz for band 3 FDD
    device_args: String,
    
    /// Number of channels
    #[arg(long, default_value = "1")]
    num_channels: usize,
    
}


/// GNodeB application state
struct GnbState {
    phy_layer: Arc<RwLock<EnhancedPhyLayer>>,
    mac_layer: Arc<EnhancedMacLayer>,
    running: Arc<RwLock<bool>>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Initialize logging
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(&args.log_level));
    
    fmt()
        .with_env_filter(env_filter)
        .with_target(true)
        .with_thread_ids(true)
        .with_line_number(true)
        .init();

    info!("Starting Albor Space 5G GNodeB");
    info!("Configuration file: {}", args.config);
    
    // Validate and create configuration
    let pci = Pci::new(args.pci)
        .ok_or_else(|| anyhow::anyhow!("Invalid PCI: {}", args.pci))?;
    
    let cell_id = CellId(args.cell_id);
    
    let bandwidth = match args.bandwidth_mhz {
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
        _ => return Err(anyhow::anyhow!("Invalid bandwidth: {} MHz", args.bandwidth_mhz)),
    };
    
    let scs = match args.scs_khz {
        15 => SubcarrierSpacing::Scs15,
        30 => SubcarrierSpacing::Scs30,
        60 => SubcarrierSpacing::Scs60,
        120 => SubcarrierSpacing::Scs120,
        240 => SubcarrierSpacing::Scs240,
        _ => return Err(anyhow::anyhow!("Invalid subcarrier spacing: {} kHz", args.scs_khz)),
    };
    
    info!("Cell configuration:");
    info!("  PCI: {}", pci.0);
    info!("  Cell ID: {}", cell_id.0);
    info!("  Frequency: {} MHz", args.frequency_mhz);
    info!("  Bandwidth: {} MHz", args.bandwidth_mhz);
    info!("  Subcarrier spacing: {} kHz", args.scs_khz);
    info!("  PHY mode: Enhanced (full)");

    // Create PHY configuration
    let phy_config = PhyConfig {
        pci,
        cell_id,
        carrier_frequency: args.frequency_mhz * 1e6,
        bandwidth,
        subcarrier_spacing: scs,
        num_tx_antennas: 1,
        num_rx_antennas: 1,
        cyclic_prefix: CyclicPrefix::Normal,
        duplex_mode: DuplexMode::Fdd,  // Band 3 is FDD
    };
    
    // Create ZMQ RF configuration from device args
    let zmq_config = ZmqRfConfig::from_device_args(&args.device_args, args.num_channels)?;
    
    info!("ZMQ configuration:");
    info!("  TX address: {}", zmq_config.tx_address);
    info!("  RX address: {}", zmq_config.rx_address);
    info!("  Sample rate: {} MHz", zmq_config.sample_rate / 1e6);

    // Create MAC configuration
    let mac_config = MacConfig {
        cell_id,
        scs,
        bandwidth,
        max_ues: 32,
        sib1_config: default_sib1_config(cell_id),
    };
    
    // Initialize MAC layer
    let mut mac_layer = EnhancedMacLayer::new(mac_config.clone())?;
    mac_layer.initialize().await?;
    info!("MAC layer initialized");
    let mac_layer = Arc::new(mac_layer);
    
    // Initialize PHY layer
    let mut enhanced_phy = EnhancedPhyLayer::new(phy_config)?;
    
    // Set MAC interface for PHY - cast to trait object
    let mac_interface: Arc<dyn layers::mac::MacPhyInterface> = mac_layer.clone() as Arc<dyn layers::mac::MacPhyInterface>;
    enhanced_phy.set_mac_interface(mac_interface);
    
    enhanced_phy.initialize_with_rf(zmq_config).await?;
    info!("Enhanced PHY layer initialized (full mode)");
    let phy_layer = Arc::new(RwLock::new(enhanced_phy));
    
    let running = Arc::new(RwLock::new(true));
    
    let state = GnbState {
        phy_layer,
        mac_layer,
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
    
    // Wait for tasks to complete
    let _ = tokio::time::timeout(
        tokio::time::Duration::from_secs(5),
        stats_handle
    ).await;
    
    info!("GNodeB shutdown complete");
    Ok(())
}