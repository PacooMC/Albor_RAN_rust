#!/bin/bash
# Final test for SSB transmission and cell detection

set -e

LOG_DIR="/workspace/logs/$(date +%Y%m%d_%H%M%S)_cell_detection"
mkdir -p "$LOG_DIR"

echo "=== Final Cell Detection Test ==="
echo "Log directory: $LOG_DIR"

# Kill any existing processes
echo "Cleaning up..."
pkill -f 'albor_gnodeb' || true
pkill -f 'srsue' || true
lsof -ti:2000 | xargs kill -9 2>/dev/null || true
lsof -ti:2001 | xargs kill -9 2>/dev/null || true
sleep 2

# Set debug logging for PHY layer
export RUST_LOG=albor_gnodeb=info,layers::phy=debug,interfaces::zmq_rf=info

# Start gNodeB
echo "Starting Albor gNodeB..."
cd /workspace
./target/release/albor_gnodeb -c config/albor_gnb/gnb_albor.yml > "$LOG_DIR/gnb.log" 2>&1 &
GNB_PID=$!

# Wait for gNodeB initialization
echo "Waiting for gNodeB initialization..."
for i in {1..10}; do
    if grep -q "PHY layer initialized\|GNodeB initialized successfully" "$LOG_DIR/gnb.log" 2>/dev/null; then
        echo "✓ gNodeB initialized"
        break
    fi
    printf "\r[%02d/10] Waiting..." "$i"
    sleep 1
done
echo ""

# Verify ZMQ is listening
echo "Checking ZMQ interfaces..."
if lsof -i:2000 | grep -q LISTEN; then
    echo "✓ ZMQ TX port 2000 is listening"
else
    echo "✗ ZMQ TX port 2000 not listening"
fi

# Start UE
echo "Starting srsUE..."
cd /opt/srsran
export LD_LIBRARY_PATH=/opt/srsran/lib:$LD_LIBRARY_PATH
/opt/srsran/bin/srsue /workspace/config/srsue/ue_nr_zmq_10mhz.conf > "$LOG_DIR/ue.log" 2>&1 &
UE_PID=$!

# Monitor for cell detection
echo "Monitoring for cell detection..."
TIMEOUT=30
for i in $(seq 1 $TIMEOUT); do
    printf "\r[%02d/%02d] Searching for cell..." "$i" "$TIMEOUT"
    
    if grep -q "Found Cell" "$LOG_DIR/ue.log" 2>/dev/null; then
        echo ""
        echo "✅ SUCCESS: UE found cell!"
        grep "Found Cell" "$LOG_DIR/ue.log"
        break
    fi
    
    if [ $i -eq $TIMEOUT ]; then
        echo ""
        echo "❌ TIMEOUT: UE did not find cell"
    fi
    
    sleep 1
done

# Show debug information
echo ""
echo "=== SSB Transmission Analysis ==="

# Count SSB transmissions
SSB_COUNT=$(grep -c "SSB transmission period" "$LOG_DIR/gnb.log" 2>/dev/null || echo "0")
PSS_COUNT=$(grep -c "Mapping PSS" "$LOG_DIR/gnb.log" 2>/dev/null || echo "0")
SSS_COUNT=$(grep -c "Mapping SSS" "$LOG_DIR/gnb.log" 2>/dev/null || echo "0")
PBCH_COUNT=$(grep -c "Mapping PBCH" "$LOG_DIR/gnb.log" 2>/dev/null || echo "0")

echo "SSB periods: $SSB_COUNT"
echo "PSS mappings: $PSS_COUNT"
echo "SSS mappings: $SSS_COUNT"
echo "PBCH mappings: $PBCH_COUNT"

# Show sample SSB logs
echo ""
echo "Sample SSB transmission logs:"
grep -E "SSB transmission period|Mapping (PSS|SSS|PBCH)" "$LOG_DIR/gnb.log" 2>/dev/null | head -10 || echo "No SSB logs found"

# Show UE logs
echo ""
echo "UE cell search logs:"
grep -E "(cell_search|Found Cell|Searching|PSS|SSS)" "$LOG_DIR/ue.log" 2>/dev/null | tail -20 || echo "No UE logs found"

# Check ZMQ communication
echo ""
echo "ZMQ communication status:"
grep -E "(ZMQ|zmq)" "$LOG_DIR/gnb.log" 2>/dev/null | grep -E "(connected|bound|TX|RX)" | tail -5 || echo "No ZMQ logs found"

# Cleanup
echo ""
echo "Stopping processes..."
kill $GNB_PID $UE_PID 2>/dev/null || true

echo ""
echo "Test complete. Full logs available in $LOG_DIR"
echo "  - gNodeB log: $LOG_DIR/gnb.log"
echo "  - UE log: $LOG_DIR/ue.log"