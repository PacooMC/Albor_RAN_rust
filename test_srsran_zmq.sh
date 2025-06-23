#!/bin/bash
# Simple test script to run gNodeB with correct parameters and srsUE

set -e

# Color codes
GREEN='\033[0;32m'
RED='\033[0;31m'
NC='\033[0m'

log_info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Create log directory
LOG_DIR="/workspace/logs/$(date +%Y%m%d_%H%M%S)_simple"
mkdir -p "$LOG_DIR"

# Clean up ports
log_info "Cleaning up ZMQ ports..."
killall -9 srsue albor_gnodeb 2>/dev/null || true
sleep 2

# Build our gNodeB
log_info "Building Albor gNodeB..."
cd /workspace
cargo build --release

# Run our gNodeB with 10 MHz bandwidth
log_info "Starting Albor gNodeB with 10 MHz bandwidth..."
RUST_LOG=debug,albor_gnodeb=trace,layers=trace,interfaces=trace \
    ./target/release/albor_gnodeb \
    --bandwidth-mhz 10 \
    --frequency-mhz 1842.5 \
    --scs-khz 15 \
    --device-args "tx_port=tcp://*:2000,rx_port=tcp://localhost:2001,base_srate=23.04e6" \
    > "$LOG_DIR/gnodeb.log" 2>&1 &
GNODEB_PID=$!

# Wait for initialization
sleep 3

# Create simple UE config for band 3, 10 MHz
cat > "$LOG_DIR/ue.conf" << 'EOF'
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
nof_prb = 52

[log]
all_level = debug
phy_level = debug
filename = /tmp/ue.log
EOF

# Run srsUE
log_info "Starting srsUE..."
/opt/srsran/bin/srsue "$LOG_DIR/ue.conf" > "$LOG_DIR/ue.log" 2>&1 &
UE_PID=$!

# Monitor for 20 seconds
log_info "Monitoring for 20 seconds..."
for i in {1..20}; do
    if ! kill -0 $GNODEB_PID 2>/dev/null; then
        log_error "gNodeB crashed!"
        break
    fi
    if ! kill -0 $UE_PID 2>/dev/null; then
        log_info "UE stopped (may be normal)"
        break
    fi
    sleep 1
    echo -n "."
done
echo ""

# Cleanup
log_info "Stopping processes..."
kill $GNODEB_PID $UE_PID 2>/dev/null || true

# Check results
echo ""
echo "=== Results ==="
if grep -q "Found Cell" "$LOG_DIR/ue.log" 2>/dev/null; then
    log_info "SUCCESS: UE detected cell!"
    grep "Found Cell" "$LOG_DIR/ue.log"
else
    log_error "FAILURE: UE did not detect cell"
fi

echo ""
echo "=== gNodeB SSB/PSS/SSS logs ==="
grep -i "ssb\|pss\|sss" "$LOG_DIR/gnodeb.log" | tail -10 || echo "No SSB logs found"

echo ""
echo "=== UE cell search logs ==="
grep -i "cell\|search\|sync" "$LOG_DIR/ue.log" | tail -10 || echo "No cell search logs found"

log_info "Logs saved in $LOG_DIR"