#!/bin/bash
# Test cell detection only (no core network)

set -e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

log_info() { echo -e "${GREEN}[INFO]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }
log_warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }

# Create log directory
LOG_DIR="/tmp/logs/$(date +%Y%m%d_%H%M%S)_cell_detection"
mkdir -p "$LOG_DIR"

log_info "=== Cell Detection Test ==="
log_info "Testing: Albor gNodeB + srsRAN UE (no core network)"
log_info "Log directory: $LOG_DIR"

# Cleanup function
cleanup() {
    log_info "Cleaning up..."
    pkill -f 'albor_gnodeb|srsue' || true
}
trap cleanup EXIT

# Kill existing processes
cleanup
sleep 2

# Build Albor gNodeB
cd /workspace
if [ ! -f target/release/albor_gnodeb ]; then
    log_info "Building Albor gNodeB..."
    cargo build --release > $LOG_DIR/build.log 2>&1
    if [ $? -ne 0 ]; then
        log_error "Build failed"
        tail -20 $LOG_DIR/build.log
        exit 1
    fi
fi

# Start Albor gNodeB without AMF
log_info "Starting Albor gNodeB (no AMF connection)..."
export RUST_LOG=info,albor=debug,layers=debug
./target/release/albor_gnodeb -c /workspace/config/albor_gnb/gnb_albor.yml > $LOG_DIR/gnb.log 2>&1 &
GNB_PID=$!
log_info "Albor gNodeB started (PID: $GNB_PID)"
sleep 3

# Start srsUE with cell search only
log_info "Starting srsUE in cell search mode..."
cd /opt/srsran
export LD_LIBRARY_PATH=/opt/srsran/lib:$LD_LIBRARY_PATH

# Create a modified config for cell search only
cat > $LOG_DIR/ue_cell_search.conf << EOF
[rf]
freq_offset = 0
tx_gain = 50
rx_gain = 40
srate = 11.52e6
nof_antennas = 1
device_name = zmq
device_args = fail_on_disconnect=true,tx_port=tcp://127.0.0.1:2001,rx_port=tcp://127.0.0.1:2000,id=ue,base_srate=11.52e6

[rat.eutra]
dl_earfcn = 2850

[rat.nr]
bands = 3
nof_carriers = 1

[pcap]
enable = none
mac_filename = /tmp/ue_mac.pcap
mac_nr_filename = /tmp/ue_mac_nr.pcap

[log]
all_level = info
phy_level = info
phy_lib_level = info
nas_level = info
rrc_level = info

[usim]
mode = soft
algo = milenage
opc  = 63BFA50EE6523365FF14C1F45F88737D
k    = 00112233445566778899aabbccddeeff
imsi = 001010000000001
imei = 353490069873319

[rrc]
release = 15
ue_category = 4

[nas]
apn = srsapn
apn_protocol = ipv4
EOF

# Run UE for 30 seconds to detect cell
timeout 30 /opt/srsran/bin/srsue $LOG_DIR/ue_cell_search.conf > $LOG_DIR/ue.log 2>&1 &
UE_PID=$!

# Monitor for cell detection
log_info "Monitoring for cell detection..."
for i in {1..25}; do
    printf "\r[%02d/25] Waiting for cell detection..." "$i"
    
    if grep -q "Found Cell" "$LOG_DIR/ue.log" 2>/dev/null; then
        echo ""
        log_info "✓ Cell detected!"
        grep "Found Cell" "$LOG_DIR/ue.log"
        
        # Show more details
        echo ""
        log_info "Cell search details:"
        grep -E "(PSS|SSS|MIB|cell_id|RSRP)" "$LOG_DIR/ue.log" | tail -20
        
        echo ""
        log_info "✅ SUCCESS: UE detected Albor gNodeB cell!"
        exit 0
    fi
    
    sleep 1
done

echo ""
log_error "❌ FAILED: UE did not detect cell"

# Show debug info
echo ""
log_info "Albor gNodeB log (PSS/SSS/PBCH entries):"
grep -E "(PSS|SSS|PBCH|Mapping|Transmit)" "$LOG_DIR/gnb.log" | tail -30

echo ""
log_info "UE PHY log:"
grep -E "(Searching|PSS|SSS|cell search|zmq)" "$LOG_DIR/ue.log" | tail -30

exit 1