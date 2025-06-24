#!/bin/bash
# Minimal cell detection test

set -e

# Create log directory
LOG_DIR="/tmp/logs/$(date +%Y%m%d_%H%M%S)_simple"
mkdir -p "$LOG_DIR"

echo "=== Simple Cell Detection Test ==="
echo "Log directory: $LOG_DIR"

# Kill existing processes
pkill -f 'albor_gnodeb|srsue' || true
sleep 2

# Build if needed
cd /workspace
if [ ! -f target/release/albor_gnodeb ]; then
    echo "Building Albor gNodeB..."
    cargo build --release > $LOG_DIR/build.log 2>&1
fi

# Start gNodeB with debug logging
echo "Starting gNodeB..."
export RUST_LOG=info,albor=debug,layers=debug
./target/release/albor_gnodeb -c /workspace/config/albor_gnb/gnb_albor.yml > $LOG_DIR/gnb.log 2>&1 &
GNB_PID=$!
echo "gNodeB PID: $GNB_PID"
sleep 5

# Start UE with sacred config
echo "Starting UE..."
cd /opt/srsran
export LD_LIBRARY_PATH=/opt/srsran/lib:$LD_LIBRARY_PATH
timeout 20 /opt/srsran/bin/srsue /workspace/config/srsue/ue_nr_zmq_10mhz.conf > $LOG_DIR/ue.log 2>&1 || true

# Check results
echo ""
if grep -q "Found Cell" "$LOG_DIR/ue.log" 2>/dev/null; then
    echo "✓ SUCCESS: Cell detected!"
    grep "Found Cell" "$LOG_DIR/ue.log"
    echo ""
    echo "Cell details:"
    grep -E "(PSS|SSS|MIB|cell_id|RSRP)" "$LOG_DIR/ue.log" | tail -20
else
    echo "✗ FAILED: Cell not detected"
    echo ""
    echo "UE log tail:"
    tail -30 "$LOG_DIR/ue.log"
    echo ""
    echo "gNodeB SSB logs:"
    grep -E "(PSS|SSS|PBCH|Mapping)" "$LOG_DIR/gnb.log" | tail -20
fi

# Cleanup
kill $GNB_PID 2>/dev/null || true
pkill -f 'srsue' || true