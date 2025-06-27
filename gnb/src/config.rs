//! YAML Configuration Structures for srsRAN-compatible format
//! 
//! These structures EXACTLY match the sacred gnb_albor.yml format

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Main configuration structure matching srsRAN YAML format
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GnbConfig {
    /// CU-CP configuration
    pub cu_cp: CuCpConfig,
    /// CU-UP configuration
    pub cu_up: CuUpConfig,
    /// RU SDR configuration
    pub ru_sdr: RuSdrConfig,
    /// Cell configuration
    pub cell_cfg: CellConfig,
    /// Logging configuration
    #[serde(default)]
    pub log: LogConfig,
    /// PCAP configuration
    #[serde(default)]
    pub pcap: PcapConfig,
}

/// CU-CP (Control Plane) configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CuCpConfig {
    /// AMF configuration
    pub amf: AmfConfig,
    /// Inactivity timer in seconds
    #[serde(default = "default_inactivity_timer")]
    pub inactivity_timer: u32,
}

fn default_inactivity_timer() -> u32 {
    7200
}

/// AMF configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AmfConfig {
    /// AMF address
    pub addr: String,
    /// AMF port
    pub port: u16,
    /// Bind address for gNodeB
    pub bind_addr: String,
    /// Supported tracking areas
    pub supported_tracking_areas: Vec<TrackingAreaConfig>,
}

/// Tracking area configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TrackingAreaConfig {
    /// Tracking area code
    pub tac: u32,
    /// PLMN list
    pub plmn_list: Vec<PlmnConfig>,
}

/// PLMN configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PlmnConfig {
    /// PLMN ID (MCC+MNC)
    pub plmn: String,
    /// TAI slice support list
    pub tai_slice_support_list: Vec<SliceConfig>,
}

/// Slice configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SliceConfig {
    /// Slice/Service Type
    pub sst: u8,
    /// Slice Differentiator (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sd: Option<u32>,
}

/// CU-UP (User Plane) configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CuUpConfig {
    /// GTP-U bind address
    pub gtpu_bind_addr: String,
    /// GTP-U external address
    pub gtpu_ext_addr: String,
}

/// RU SDR configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RuSdrConfig {
    /// Device driver type
    pub device_driver: String,
    /// Device arguments
    pub device_args: String,
    /// Sample rate in MHz
    pub srate: f64,
    /// Transmit gain in dB
    pub tx_gain: f32,
    /// Receive gain in dB
    pub rx_gain: f32,
}

/// Cell configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CellConfig {
    /// Downlink ARFCN
    pub dl_arfcn: u32,
    /// Band number
    pub band: u16,
    /// Channel bandwidth in MHz
    #[serde(rename = "channel_bandwidth_MHz")]
    pub channel_bandwidth_mhz: u32,
    /// Common subcarrier spacing in kHz
    pub common_scs: u32,
    /// PLMN
    pub plmn: String,
    /// Tracking area code
    pub tac: u32,
    /// Physical Cell ID
    pub pci: u16,
    /// PDCCH configuration
    pub pdcch: PdcchConfig,
    /// PRACH configuration
    pub prach: PrachConfig,
    /// PDSCH configuration
    #[serde(default)]
    pub pdsch: PdschConfig,
    /// PUSCH configuration
    #[serde(default)]
    pub pusch: PuschConfig,
}

/// PDCCH configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PdcchConfig {
    /// Common PDCCH configuration
    pub common: CommonPdcchConfig,
    /// Dedicated PDCCH configuration
    #[serde(default)]
    pub dedicated: DedicatedPdcchConfig,
}

/// Common PDCCH configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CommonPdcchConfig {
    /// Search space 0 index
    pub ss0_index: u8,
    /// CORESET#0 index
    pub coreset0_index: u8,
}

/// Dedicated PDCCH configuration
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct DedicatedPdcchConfig {
    /// Search space 2 type
    #[serde(default = "default_ss2_type")]
    pub ss2_type: String,
    /// DCI format 0_1 and 1_1 enabled
    #[serde(default)]
    pub dci_format_0_1_and_1_1: bool,
}

fn default_ss2_type() -> String {
    "common".to_string()
}

/// PRACH configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PrachConfig {
    /// PRACH configuration index
    pub prach_config_index: u8,
    /// PRACH root sequence index
    pub prach_root_sequence_index: u16,
    /// Zero correlation zone
    pub zero_correlation_zone: u8,
    /// PRACH frequency start
    pub prach_frequency_start: u16,
    /// Total number of RA preambles
    #[serde(default = "default_total_nof_ra_preambles")]
    pub total_nof_ra_preambles: u8,
    /// Number of SSB per RACH occasion
    #[serde(default = "default_nof_ssb_per_ro")]
    pub nof_ssb_per_ro: u8,
    /// Number of CB preambles per SSB
    #[serde(default = "default_nof_cb_preambles_per_ssb")]
    pub nof_cb_preambles_per_ssb: u8,
}

fn default_total_nof_ra_preambles() -> u8 {
    64  // Standard value for PRACH
}

fn default_nof_ssb_per_ro() -> u8 {
    1  // One SSB per RACH occasion
}

fn default_nof_cb_preambles_per_ssb() -> u8 {
    64  // All preambles are CB preambles by default
}

/// PDSCH configuration
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct PdschConfig {
    /// MCS table
    #[serde(default = "default_mcs_table")]
    pub mcs_table: String,
}

/// PUSCH configuration
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct PuschConfig {
    /// MCS table
    #[serde(default = "default_mcs_table")]
    pub mcs_table: String,
}

fn default_mcs_table() -> String {
    "qam64".to_string()
}

/// Logging configuration
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct LogConfig {
    /// Log filename
    pub filename: Option<String>,
    /// All layers log level
    #[serde(default = "default_log_level")]
    pub all_level: String,
    /// PHY layer log level
    #[serde(default = "default_log_level")]
    pub phy_level: String,
    /// MAC layer log level
    #[serde(default = "default_log_level")]
    pub mac_level: String,
    /// RLC layer log level
    #[serde(default = "default_log_level")]
    pub rlc_level: String,
    /// PDCP layer log level
    #[serde(default = "default_log_level")]
    pub pdcp_level: String,
    /// RRC layer log level
    #[serde(default = "default_log_level")]
    pub rrc_level: String,
    /// NGAP layer log level
    #[serde(default = "default_log_level")]
    pub ngap_level: String,
}

fn default_log_level() -> String {
    "info".to_string()
}

/// PCAP configuration
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct PcapConfig {
    /// Enable MAC PCAP
    #[serde(default)]
    pub mac_enable: bool,
    /// MAC PCAP filename
    pub mac_filename: Option<String>,
    /// Enable NGAP PCAP
    #[serde(default)]
    pub ngap_enable: bool,
    /// NGAP PCAP filename
    pub ngap_filename: Option<String>,
}

impl GnbConfig {
    /// Load configuration from YAML file
    pub fn from_yaml_file(path: &str) -> anyhow::Result<Self> {
        let contents = std::fs::read_to_string(path)?;
        let config: GnbConfig = serde_yaml::from_str(&contents)?;
        Ok(config)
    }
    
    /// Parse PLMN string (e.g., "00101") into MCC and MNC components
    pub fn parse_plmn(plmn: &str) -> anyhow::Result<(u16, u16)> {
        if plmn.len() < 5 || plmn.len() > 6 {
            return Err(anyhow::anyhow!("Invalid PLMN format: {}", plmn));
        }
        
        let mcc = plmn[0..3].parse::<u16>()?;
        let mnc = plmn[3..].parse::<u16>()?;
        
        Ok((mcc, mnc))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_parse_plmn() {
        // Test 5-digit PLMN
        let (mcc, mnc) = GnbConfig::parse_plmn("00101").unwrap();
        assert_eq!(mcc, 1);
        assert_eq!(mnc, 1);
        
        // Test 6-digit PLMN
        let (mcc, mnc) = GnbConfig::parse_plmn("310260").unwrap();
        assert_eq!(mcc, 310);
        assert_eq!(mnc, 260);
    }
}