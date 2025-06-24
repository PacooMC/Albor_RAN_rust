#!/bin/bash
# Test cell detection with reference srsRAN gNodeB

set -e

# Create log directory
LOG_DIR="/tmp/logs/$(date +%Y%m%d_%H%M%S)_srsran_cell"
mkdir -p "$LOG_DIR"

echo "=== srsRAN Cell Detection Test (Reference) ==="
echo "Log directory: $LOG_DIR"

# Kill existing processes
pkill -f 'gnb|srsue' || true
sleep 2

# Start srsRAN gNodeB
echo "Starting srsRAN gNodeB..."
cd /opt/srsran_project
./bin/gnb -c /workspace/config/srsran_gnb/gnb_zmq_10mhz.yml > $LOG_DIR/gnb.log 2>&1 &
GNB_PID=$!
echo "srsRAN gNodeB PID: $GNB_PID"
sleep 5

# Start UE
echo "Starting srsUE..."
cd /opt/srsran
export LD_LIBRARY_PATH=/opt/srsran/lib:$LD_LIBRARY_PATH
timeout 20 /opt/srsran/bin/srsue /workspace/config/srsue/ue_nr_zmq_10mhz.conf > $LOG_DIR/ue.log 2>&1 || true

# Check results
echo ""
if grep -q "Found Cell" "$LOG_DIR/ue.log" 2>/dev/null; then
    echo "✓ SUCCESS: Cell detected with srsRAN gNodeB!"
    grep "Found Cell" "$LOG_DIR/ue.log"
    echo ""
    echo "Cell details:"
    grep -E "(PSS|SSS|MIB|cell_id|RSRP)" "$LOG_DIR/ue.log" | tail -20
else
    echo "✗ FAILED: Cell not detected even with srsRAN"
    echo ""
    echo "UE log tail:"
    tail -30 "$LOG_DIR/ue.log"
fi

# Cleanup
kill $GNB_PID 2>/dev/null || true
pkill -f 'srsue' || true