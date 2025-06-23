#!/bin/bash
# Test script for gNodeB standalone operation (no 5G core)
# Focus on PSS/SSS detection and cell search

set -e

# Color codes
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

# Logging functions
log_info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

log_debug() {
    echo -e "${BLUE}[DEBUG]${NC} $1"
}

# Create dated log directory
LOG_DIR="/workspace/logs/$(date +%Y%m%d_%H%M%S)_standalone"
mkdir -p "$LOG_DIR"
log_info "Created log directory: $LOG_DIR"

# Clean up any processes using ZMQ ports
log_info "Cleaning up ZMQ ports 2000 and 2001..."
killall -9 srsue 2>/dev/null || true
killall -9 gnb 2>/dev/null || true
killall -9 albor_gnodeb 2>/dev/null || true

# Check and kill processes on ports
PORT_2000_PID=$(ss -tlnp 2>/dev/null | grep ':2000' | grep -oE 'pid=[0-9]+' | cut -d= -f2 | head -1)
if [ ! -z "$PORT_2000_PID" ]; then
    log_warn "Port 2000 is in use by PID $PORT_2000_PID. Terminating..."
    kill -9 $PORT_2000_PID 2>/dev/null || true
fi

PORT_2001_PID=$(ss -tlnp 2>/dev/null | grep ':2001' | grep -oE 'pid=[0-9]+' | cut -d= -f2 | head -1)
if [ ! -z "$PORT_2001_PID" ]; then
    log_warn "Port 2001 is in use by PID $PORT_2001_PID. Terminating..."
    kill -9 $PORT_2001_PID 2>/dev/null || true
fi

sleep 2

# Parse command line arguments
USE_REFERENCE=false
while [[ $# -gt 0 ]]; do
    case $1 in
        --use-reference)
            USE_REFERENCE=true
            shift
            ;;
        *)
            echo "Unknown option: $1"
            echo "Usage: $0 [--use-reference]"
            exit 1
            ;;
    esac
done

# Step 1: Compile our GNodeB if not using reference
if [ "$USE_REFERENCE" = false ]; then
    log_info "Compiling Albor Space GNodeB project..."
    cd /workspace
    RUST_LOG=debug cargo build --release 2>&1 | tee "$LOG_DIR/build.log"
    if [ ${PIPESTATUS[0]} -ne 0 ]; then
        log_error "Build failed! Check $LOG_DIR/build.log for details"
        exit 1
    fi
    log_info "Build completed successfully"
fi

# Step 2: Start GNodeB (our implementation or reference)
if [ "$USE_REFERENCE" = true ]; then
    log_info "Starting REFERENCE srsRAN gNodeB (standalone mode)..."
    
    # Create standalone gNodeB configuration (no AMF)
    cat > "$LOG_DIR/gnb_zmq_standalone.yml" << 'EOF'
# Standalone gNodeB configuration - no AMF connection
# Based on /workspace/config/srsran_gnb/gnb_zmq.yml

# srsRAN gNodeB configuration file

cu_cp:
  # No AMF connection for standalone testing

ru_sdr:
  device_driver: zmq
  device_args: fail_on_disconnect=true,tx_port=tcp://127.0.0.1:2000,rx_port=tcp://127.0.0.1:2001,base_srate=23.04e6
  srate: 23.04e6
  tx_gain: 75
  rx_gain: 75

cell_cfg:
  dl_arfcn: 368500                  # Band 3, 1842.5 MHz
  band: 3
  channel_bandwidth_MHz: 10          # 52 PRBs
  common_scs: 15                     # 15 kHz subcarrier spacing
  plmn: "00101"
  tac: 7
  pci: 1                             # Physical Cell ID
  prach:
    prach_config_index: 1            # FDD PRACH

log:
  filename: /tmp/gnb_standalone.log
  all_level: info
  phy_level: debug                  # Enable PHY debug for signal analysis
  mac_level: debug
  hex_max_size: 512
EOF

    log_info "Starting reference gNodeB with standalone config..."
    /opt/srsran_project/bin/gnb -c "$LOG_DIR/gnb_zmq_standalone.yml" > "$LOG_DIR/reference_gnb.log" 2>&1 &
    GNODEB_PID=$!
    log_info "Reference gNodeB started with PID: $GNODEB_PID"
    
else
    log_info "Starting Albor Space GNodeB (standalone mode)..."
    cd /workspace
    RUST_LOG=debug,albor_gnodeb=trace,layers=trace,interfaces=trace ./target/release/albor_gnodeb > "$LOG_DIR/gnodeb.log" 2>&1 &
    GNODEB_PID=$!
    log_info "Albor GNodeB started with PID: $GNODEB_PID"
fi

# Start tailing gNodeB logs
tail -f "$LOG_DIR"/*gnb*.log | grep -E "(PSS|SSS|SSB|PBCH|symbol|slot|frame|power|amplitude|transmitted)" | sed 's/^/[GNB] /' &
TAIL_GNB_PID=$!

# Wait for GNodeB to initialize
sleep 3

# Check if GNodeB is still running
if ! kill -0 $GNODEB_PID 2>/dev/null; then
    log_error "GNodeB failed to start. Check logs in $LOG_DIR"
    cat "$LOG_DIR"/*gnb*.log | tail -50
    exit 1
fi

# Step 3: Create enhanced UE configuration for cell search debugging
log_info "Creating UE configuration with maximum debugging..."
cat > "$LOG_DIR/ue_zmq_debug.conf" << 'EOF'
[rf]
freq_offset = 0
tx_gain = 50
rx_gain = 40
srate = 23.04e6
nof_antennas = 1

device_name = zmq
device_args = fail_on_disconnect=true,tx_port0=tcp://*:2001,rx_port0=tcp://localhost:2000,base_srate=23.04e6

[rat.eutra]
dl_earfcn = 3350
nof_carriers = 0

[rat.nr]
bands = 3
nof_carriers = 1
max_nof_prb = 52         # 10 MHz bandwidth
nof_prb = 52

[rrc]
release = 15
ue_category = 4
nr_measurement_pci = 1
nr_short_sn_support = true

[pcap]
enable = mac_nr
mac_filename = /tmp/ue_mac.pcap
mac_nr_filename = /tmp/ue_mac_nr.pcap
nas_filename = /tmp/ue_nas.pcap

[log]
all_level = debug
phy_level = debug        # Maximum PHY debugging
phy_lib_level = debug    # PHY library debugging
mac_level = debug
rlc_level = info
rrc_level = debug
nas_level = info
all_hex_limit = 512
filename = /tmp/ue_debug.log
file_max_size = -1

[usim]
mode = soft
algo = milenage
opc  = 63BFA50EE6523365FF14C1F45F88737D
k    = 00112233445566778899aabbccddeeff
imsi = 001010000000001
imei = 353490069873319

[nas]
apn = internet
apn_protocol = ipv4

[gw]
netns = 
ip_devname = tun_srsue
ip_netmask = 255.255.255.0

[gui]
enable = false
EOF

# Step 4: Start UE with debugging
log_info "Starting srsUE with maximum debugging..."
export LD_LIBRARY_PATH=/opt/srsran/lib:$LD_LIBRARY_PATH
/opt/srsran/bin/srsue "$LOG_DIR/ue_zmq_debug.conf" > "$LOG_DIR/ue.log" 2>&1 &
UE_PID=$!
log_info "srsUE started with PID: $UE_PID"

# Start tailing UE logs with focus on cell search
tail -f "$LOG_DIR/ue.log" | grep -E "(PSS|SSS|cell|search|detect|sync|RSRP|RSRQ|SNR|power|Found|Cell)" | sed 's/^/[UE] /' &
TAIL_UE_PID=$!

# Let the test run for 30 seconds for thorough cell search
log_info "Running test for 30 seconds to allow complete cell search..."
for i in {1..30}; do
    echo -n "."
    sleep 1
    # Check if processes are still running
    if ! kill -0 $GNODEB_PID 2>/dev/null; then
        log_error "GNodeB crashed!"
        break
    fi
    if ! kill -0 $UE_PID 2>/dev/null; then
        log_warn "UE terminated (this may be normal if no cell found)"
        break
    fi
done
echo ""

# Kill tailing processes
kill $TAIL_GNB_PID 2>/dev/null || true
kill $TAIL_UE_PID 2>/dev/null || true

# Terminate main processes
log_info "Terminating processes..."
kill -TERM $GNODEB_PID 2>/dev/null || true
kill -TERM $UE_PID 2>/dev/null || true
sleep 2
kill -KILL $GNODEB_PID 2>/dev/null || true
kill -KILL $UE_PID 2>/dev/null || true

# Step 5: Analyze results
log_info "Analyzing test results..."

echo "=== PSS/SSS Detection Analysis ===" > "$LOG_DIR/analysis.txt"
echo "Test Date: $(date)" >> "$LOG_DIR/analysis.txt"
echo "Mode: $([ "$USE_REFERENCE" = true ] && echo "Reference srsRAN gNodeB" || echo "Albor gNodeB")" >> "$LOG_DIR/analysis.txt"
echo "" >> "$LOG_DIR/analysis.txt"

# Check GNodeB transmission
echo "=== GNodeB PSS/SSS Transmission ===" >> "$LOG_DIR/analysis.txt"
grep -i "pss\|sss\|ssb" "$LOG_DIR"/*gnb*.log | tail -20 >> "$LOG_DIR/analysis.txt" || echo "No PSS/SSS logs found" >> "$LOG_DIR/analysis.txt"
echo "" >> "$LOG_DIR/analysis.txt"

# Check UE cell search
echo "=== UE Cell Search Results ===" >> "$LOG_DIR/analysis.txt"
grep -i "cell.*search\|found.*cell\|pss\|sss\|sync" "$LOG_DIR/ue.log" | tail -20 >> "$LOG_DIR/analysis.txt" || echo "No cell search logs found" >> "$LOG_DIR/analysis.txt"
echo "" >> "$LOG_DIR/analysis.txt"

# Check for successful detection
echo "=== Detection Status ===" >> "$LOG_DIR/analysis.txt"
if grep -q "Found Cell" "$LOG_DIR/ue.log"; then
    echo "SUCCESS: UE detected cell!" >> "$LOG_DIR/analysis.txt"
    grep "Found Cell" "$LOG_DIR/ue.log" >> "$LOG_DIR/analysis.txt"
else
    echo "FAILURE: UE did not detect any cells" >> "$LOG_DIR/analysis.txt"
fi
echo "" >> "$LOG_DIR/analysis.txt"

# Extract power/timing info
echo "=== Signal Parameters ===" >> "$LOG_DIR/analysis.txt"
grep -E "power|amplitude|gain|timing" "$LOG_DIR"/*gnb*.log | tail -10 >> "$LOG_DIR/analysis.txt" || true
echo "" >> "$LOG_DIR/analysis.txt"

# Display analysis
cat "$LOG_DIR/analysis.txt"

log_info "Test completed. All logs saved in $LOG_DIR"
log_info "Key files:"
log_info "  - gnodeb.log: Complete gNodeB output"
log_info "  - ue.log: Complete UE output"
log_info "  - analysis.txt: Test result analysis"

# Exit with appropriate code
if grep -q "Found Cell" "$LOG_DIR/ue.log"; then
    log_info "Test PASSED - UE detected cell"
    exit 0
else
    log_error "Test FAILED - UE did not detect cell"
    exit 1
fi