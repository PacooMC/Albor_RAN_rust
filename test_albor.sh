#!/bin/bash
# Test Albor gNodeB + srsRAN UE
# This tests our implementation against the reference UE

set -e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

log_info() { echo -e "${GREEN}[INFO]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }
log_warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }

# No more command line arguments needed

# Create log directory
LOG_DIR="logs/$(date +%Y%m%d_%H%M%S)_albor"
mkdir -p "$LOG_DIR"

log_info "=== Albor gNodeB Test ==="
log_info "Testing Albor gNodeB (ENHANCED PHY) + srsRAN UE"
log_info "Log directory: $LOG_DIR"

# Cleanup function
cleanup() {
    log_info "Cleaning up..."
    docker exec albor-gnb-dev bash -c "pkill -f 'albor_gnodeb|srsue' || true" 2>/dev/null || true
}
trap cleanup EXIT

# Kill existing processes
log_info "Stopping any existing processes..."
cleanup
sleep 1

# Check if in Docker
if [ -z "$IN_DOCKER" ]; then
    log_info "Running test inside Docker container..."
    docker exec -e IN_DOCKER=1 albor-gnb-dev /workspace/test_albor.sh
    exit $?
fi

# We're inside Docker now
cd /workspace

# Build if needed
if [ ! -f target/release/albor_gnodeb ]; then
    log_info "Building Albor gNodeB..."
    cargo build --release > "$LOG_DIR/build.log" 2>&1
    if [ $? -ne 0 ]; then
        log_error "Build failed"
        tail -20 "$LOG_DIR/build.log"
        exit 1
    fi
fi

# Create UE config with debug logs
cat > /tmp/ue_test.conf << 'EOF'
[rf]
freq_offset = 0
tx_gain = 50
rx_gain = 40
srate = 23.04e6
nof_antennas = 1

device_name = zmq
device_args = tx_port=tcp://127.0.0.1:2001,rx_port=tcp://127.0.0.1:2000,id=ue,base_srate=23.04e6

[rat.eutra]
nof_carriers = 0

[rat.nr]
bands = 3
nof_carriers = 1

[pcap]
enable = none

[log]
all_level = debug
phy_level = debug
phy_lib_level = info
mac_level = debug
rrc_level = debug
filename = /tmp/ue.log
file_max_size = -1

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
apn = internet
apn_protocol = ipv4

[gw]
ip_devname = tun_srsue
ip_netmask = 255.255.255.0

[gui]
enable = false
EOF

# Start Albor gNodeB
log_info "Starting Albor gNodeB..."
GNODEB_ARGS="--pci 1"

./target/release/albor_gnodeb $GNODEB_ARGS > "$LOG_DIR/gnb.log" 2>&1 &
GNB_PID=$!
log_info "Albor gNodeB started (PID: $GNB_PID)"

sleep 3

# Check if gNodeB started
if ! kill -0 $GNB_PID 2>/dev/null; then
    log_error "Albor gNodeB failed to start"
    log_info "Last lines of gNodeB log:"
    tail -20 "$LOG_DIR/gnb.log"
    exit 1
fi

# Verify PHY mode
if grep -q "PHY mode: Enhanced" "$LOG_DIR/gnb.log"; then
    log_info "✓ Enhanced PHY initialized"
else
    log_error "✗ Enhanced PHY not initialized"
    exit 1
fi

# Start srsUE
log_info "Starting srsUE..."
export LD_LIBRARY_PATH=/opt/srsran/lib:$LD_LIBRARY_PATH
/opt/srsran/bin/srsue /tmp/ue_test.conf > "$LOG_DIR/ue.log" 2>&1 &
UE_PID=$!
log_info "srsUE started (PID: $UE_PID)"

# Monitor for 30 seconds
log_info "Monitoring for cell detection (30s)..."
FOUND_CELL=false
for i in {1..30}; do
    echo -n "."
    sleep 1
    
    if grep -q "Found Cell" "$LOG_DIR/ue.log" 2>/dev/null; then
        echo ""
        log_info "✓ CELL DETECTED!"
        FOUND_CELL=true
        break
    fi
done
echo ""

# Results
echo "=========================================="
log_info "TEST RESULTS:"

if [ "$FOUND_CELL" = true ]; then
    log_info "✓ SUCCESS: UE detected Albor gNodeB!"
    grep "Found Cell" "$LOG_DIR/ue.log" | tail -1
    
    # Check for additional progress
    if grep -q "Random Access" "$LOG_DIR/ue.log" 2>/dev/null; then
        log_info "✓ PRACH initiated"
    fi
else
    log_error "✗ FAILED: Cell not detected"
    
    # Debug info
    if grep -q "Cell search" "$LOG_DIR/ue.log" 2>/dev/null; then
        log_warn "UE is searching but cannot find cell"
    fi
fi

# Show Albor status
TX_COUNT=$(grep -c "TX: Received request" "$LOG_DIR/gnb.log" 2>/dev/null || echo "0")
log_info "TX requests handled: $TX_COUNT"


# Debug: Show PSS/SSS info
if grep -q "PSS sequence" "$LOG_DIR/gnb.log" 2>/dev/null; then
    log_info "PSS generation info:"
    grep "PSS sequence" "$LOG_DIR/gnb.log" | head -1
fi

echo "=========================================="
log_info "Logs: $LOG_DIR/gnb.log and $LOG_DIR/ue.log"

# Show UE debug info if no cell found
if [ "$FOUND_CELL" = false ]; then
    log_info "UE PHY debug (last 10 lines):"
    grep -i "phy\|pss\|sss\|sync" "$LOG_DIR/ue.log" 2>/dev/null | tail -10 || echo "No PHY debug logs found"
fi