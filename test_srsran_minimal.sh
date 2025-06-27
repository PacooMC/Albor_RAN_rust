#!/bin/bash
# test_srsran_minimal.sh - Test minimal srsRAN gNodeB configuration (PHY only)

set -e

LOG_DIR="./logs/$(date +%Y%m%d_%H%M%S)_srsran_minimal"
mkdir -p "$LOG_DIR"

echo "[INFO] === srsRAN Minimal PHY Test ==="
echo "[INFO] Log directory: $LOG_DIR"

# Cleanup any existing processes
echo "[INFO] Cleaning up..."
docker exec albor-gnb-dev bash -c "pkill -9 gnb 2>/dev/null || true; pkill -9 srsue 2>/dev/null || true"
sleep 2

# Start gNodeB with minimal config
echo "[INFO] Starting srsRAN gNodeB with minimal config..."
docker exec -d albor-gnb-dev bash -c "cd /opt/srsran_project && /opt/srsran_project/bin/gnb -c /workspace/config/srsran_gnb/srsran_minimal.yml > /tmp/gnb_stdout.log 2>&1"

# Wait for initialization
echo "[INFO] Waiting for gNodeB initialization..."
sleep 3

# Check if gNodeB is running
if docker exec albor-gnb-dev pgrep gnb > /dev/null; then
    echo "[INFO] ✓ gNodeB process is running"
    
    # Check for SSB transmission in logs
    echo "[INFO] Checking for SSB transmission..."
    if docker exec albor-gnb-dev grep -i "ssb\|SSB" /tmp/gnb_stdout.log 2>/dev/null; then
        echo "[INFO] ✓ SSB related messages found"
    fi
    
    # Check for cell activation
    if docker exec albor-gnb-dev grep -i "cell.*activated\|started" /tmp/gnb_stdout.log 2>/dev/null; then
        echo "[INFO] ✓ Cell activation detected"
    fi
else
    echo "[ERROR] gNodeB failed to start"
    echo "[ERROR] Startup log:"
    docker exec albor-gnb-dev cat /tmp/gnb_stdout.log 2>/dev/null || echo "No log available"
fi

# Start UE to test cell detection
echo ""
echo "[INFO] Starting srsUE for cell detection test..."
docker exec -d albor-gnb-dev bash -c "
    cd /opt/srsran
    export LD_LIBRARY_PATH=/opt/srsran/lib:\$LD_LIBRARY_PATH
    /opt/srsran/bin/srsue /workspace/config/srsue/ue_nr_zmq_10mhz.conf > /tmp/ue.log 2>&1
"

# Monitor for cell detection (shorter timeout since no RRC expected)
echo "[INFO] Monitoring for cell detection (10 seconds)..."
for i in {1..10}; do
    if docker exec albor-gnb-dev grep -q "Found Cell" /tmp/ue.log 2>/dev/null; then
        echo "[INFO] ✓ Cell detected by UE!"
        # Show cell info
        docker exec albor-gnb-dev grep -A2 "Found Cell" /tmp/ue.log 2>/dev/null || true
        break
    fi
    printf "\r[%02d/10] Waiting for cell detection..." "$i"
    sleep 1
done
echo ""

# Copy logs
echo ""
echo "[INFO] Copying logs..."
docker exec albor-gnb-dev cat /tmp/gnb_stdout.log > "$LOG_DIR/gnb_stdout.log" 2>/dev/null || true
docker exec albor-gnb-dev cat /tmp/gnb.log > "$LOG_DIR/gnb.log" 2>/dev/null || true
docker exec albor-gnb-dev cat /tmp/ue.log > "$LOG_DIR/ue.log" 2>/dev/null || true
cp config/srsran_gnb/srsran_minimal.yml "$LOG_DIR/config_used.yml" 2>/dev/null || true

# Show PHY layer activity
echo ""
echo "=== PHY Layer Status ==="
echo "From gNodeB stdout:"
docker exec albor-gnb-dev grep -i "phy\|cell\|ssb\|radio" /tmp/gnb_stdout.log 2>/dev/null | tail -10 || echo "No PHY logs found"

echo ""
echo "From gNodeB log file:"
docker exec albor-gnb-dev grep -i "phy.*tx\|phy.*rx" /tmp/gnb.log 2>/dev/null | tail -5 || echo "No PHY TX/RX logs"

echo ""
echo "=== UE Cell Search Status ==="
docker exec albor-gnb-dev grep -E "Cell|PSS|SSS|PBCH|MIB" /tmp/ue.log 2>/dev/null | tail -10 || echo "No cell search logs"

# Cleanup
echo ""
echo "[INFO] Cleaning up..."
docker exec albor-gnb-dev bash -c "pkill -9 gnb 2>/dev/null || true; pkill -9 srsue 2>/dev/null || true"

echo ""
echo "[INFO] Test complete. Logs saved to $LOG_DIR"
echo "[INFO] Check $LOG_DIR/gnb_stdout.log for detailed startup messages"