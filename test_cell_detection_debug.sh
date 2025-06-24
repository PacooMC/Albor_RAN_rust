#!/bin/bash
# Debug test for SSB transmission

LOG_DIR="logs/$(date +%Y%m%d_%H%M%S)_ssb_debug"
mkdir -p "$LOG_DIR"

echo "Starting Albor gNodeB with debug logging..."
echo "Log directory: $LOG_DIR"

# Kill any existing processes
pkill -f 'albor_gnodeb' || true
pkill -f 'srsue' || true
sleep 2

# Set debug logging for PHY layer
export RUST_LOG=albor_gnodeb=info,layers::phy=debug,interfaces::zmq_rf=debug

# Start gNodeB
echo "Starting gNodeB..."
./target/release/albor_gnodeb -c config/albor_gnb/gnb_albor.yml > "$LOG_DIR/gnb.log" 2>&1 &
GNB_PID=$!

# Wait a bit for initialization
sleep 3

# Start UE
echo "Starting UE..."
cd /opt/srsran
export LD_LIBRARY_PATH=/opt/srsran/lib:$LD_LIBRARY_PATH
/opt/srsran/bin/srsue /workspace/config/srsue/ue_nr_zmq_10mhz.conf > "$LOG_DIR/ue.log" 2>&1 &
UE_PID=$!

# Monitor for 10 seconds
echo "Monitoring for cell detection..."
for i in {1..10}; do
    echo -n "."
    if grep -q "Found Cell" "$LOG_DIR/ue.log" 2>/dev/null; then
        echo ""
        echo "SUCCESS: UE found cell!"
        break
    fi
    sleep 1
done
echo ""

# Check SSB transmission logs
echo "=== SSB Transmission Debug ==="
echo "SSB periods:"
grep "SSB transmission period" "$LOG_DIR/gnb.log" | head -5 || echo "No SSB period logs found"

echo ""
echo "PSS mapping:"
grep "Mapping PSS" "$LOG_DIR/gnb.log" | head -5 || echo "No PSS mapping logs found"

echo ""
echo "SSS mapping:"
grep "Mapping SSS" "$LOG_DIR/gnb.log" | head -5 || echo "No SSS mapping logs found"

echo ""
echo "PBCH mapping:"
grep "Mapping PBCH" "$LOG_DIR/gnb.log" | head -5 || echo "No PBCH mapping logs found"

echo ""
echo "UE status:"
grep -E "(Found Cell|Searching cell|cell_search)" "$LOG_DIR/ue.log" | tail -10 || echo "No cell search logs found"

# Cleanup
kill $GNB_PID $UE_PID 2>/dev/null || true

echo ""
echo "Debug test complete. Full logs in $LOG_DIR"