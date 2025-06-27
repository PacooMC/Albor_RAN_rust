#!/bin/bash
# test_srsran_basic.sh - Basic test for srsRAN gNodeB + UE without full Open5GS
# This tests if the reference configuration can at least achieve cell detection

set -e

# Configuration
CONTAINER_NAME="albor-gnb-dev"
LOG_DIR="./logs/$(date +%Y%m%d_%H%M%S)_srsran_basic"

# Create log directory on host
mkdir -p "$LOG_DIR"

echo "[$(date +%H:%M:%S)] Starting srsRAN basic test (20 second duration)"
echo "[$(date +%H:%M:%S)] Log directory: $LOG_DIR"

# Step 1: Clean up any existing processes
echo "[$(date +%H:%M:%S)] Cleaning up previous runs..."
docker exec $CONTAINER_NAME bash -c "pkill -9 gnb 2>/dev/null || true"
docker exec $CONTAINER_NAME bash -c "pkill -9 srsue 2>/dev/null || true"
docker exec $CONTAINER_NAME bash -c "rm -f /tmp/ue_stdin 2>/dev/null || true"
docker exec $CONTAINER_NAME bash -c "pkill -9 -f 'sleep infinity' 2>/dev/null || true"
sleep 1

# Step 2: Start srsRAN gNodeB in background
echo "[$(date +%H:%M:%S)] Starting srsRAN gNodeB..."
docker exec -d $CONTAINER_NAME bash -c "/opt/srsran_project/bin/gnb -c /workspace/config/srsran_gnb/gnb_zmq_10mhz.yml > /tmp/srsgnb.log 2>&1"

# Wait for gNodeB to initialize
echo "[$(date +%H:%M:%S)] Waiting 5 seconds for gNodeB initialization..."
sleep 5

# Step 3: Start UE with stdin pipe to prevent immediate exit
echo "[$(date +%H:%M:%S)] Starting srsUE..."
# Create a named pipe and keep it open to prevent stdin EOF
docker exec $CONTAINER_NAME bash -c "mkfifo /tmp/ue_stdin 2>/dev/null || true"
docker exec -d $CONTAINER_NAME bash -c "sleep infinity > /tmp/ue_stdin"
# Start UE with stdin from the pipe
docker exec -d $CONTAINER_NAME bash -c "export LD_LIBRARY_PATH=/opt/srsran/lib:\$LD_LIBRARY_PATH && /opt/srsran/bin/srsue /workspace/config/srsue/ue_nr_zmq_10mhz.conf < /tmp/ue_stdin > /tmp/ue.log 2>&1"

# Step 4: Monitor for 15 seconds
echo "[$(date +%H:%M:%S)] Monitoring for cell detection..."
for i in {1..15}; do
    echo -n "."
    sleep 1
done
echo ""

# Step 5: Copy logs to host
echo "[$(date +%H:%M:%S)] Copying logs..."
docker exec $CONTAINER_NAME cat /tmp/srsgnb.log > "$LOG_DIR/srsgnb.log" 2>&1 || true
docker exec $CONTAINER_NAME cat /tmp/ue.log > "$LOG_DIR/srsue.log" 2>&1 || true

# Step 6: Check results
echo "[$(date +%H:%M:%S)] Checking results..."
echo ""
echo "=== srsRAN gNodeB Status ==="
docker exec $CONTAINER_NAME bash -c "tail -20 /tmp/srsgnb.log" | grep -E "(Cell|RRC|Connected|Error)" || echo "No relevant gNodeB messages"
echo ""
echo "=== srsUE Status ==="
docker exec $CONTAINER_NAME bash -c "tail -20 /tmp/ue.log" | grep -E "(Found|Cell|RRC|Attach|Search|Error)" || echo "No relevant UE messages"

# Step 7: Stop all processes
echo ""
echo "[$(date +%H:%M:%S)] Stopping test..."
docker exec $CONTAINER_NAME bash -c "pkill -9 gnb 2>/dev/null || true"
docker exec $CONTAINER_NAME bash -c "pkill -9 srsue 2>/dev/null || true"
docker exec $CONTAINER_NAME bash -c "pkill -9 -f 'sleep infinity' 2>/dev/null || true"
docker exec $CONTAINER_NAME bash -c "rm -f /tmp/ue_stdin 2>/dev/null || true"

echo "[$(date +%H:%M:%S)] Test complete. Logs saved to $LOG_DIR"
echo ""
echo "To view full logs:"
echo "  gNodeB: cat $LOG_DIR/srsgnb.log"
echo "  UE:     cat $LOG_DIR/srsue.log"