#!/bin/bash
# test_srsran_nocore_yaml.sh - Test srsRAN gNodeB in no-core mode using YAML config

set -e

LOG_DIR="./logs/$(date +%Y%m%d_%H%M%S)_srsran_nocore_yaml"
mkdir -p "$LOG_DIR"

echo "[INFO] === srsRAN No-Core Test (YAML) ==="
echo "[INFO] Log directory: $LOG_DIR"

# Cleanup
echo "[INFO] Cleaning up..."
docker exec albor-gnb-dev bash -c "pkill -9 gnb 2>/dev/null || true; pkill -9 srsue 2>/dev/null || true"
sleep 2

# Start gNodeB with YAML config
echo "[INFO] Starting srsRAN gNodeB with no_core YAML config..."
docker exec -d albor-gnb-dev bash -c "cd /opt/srsran_project && /opt/srsran_project/bin/gnb -c /workspace/config/srsran_gnb/gnb_zmq_nocore.yml > /tmp/gnb_stdout.log 2>&1"

# Wait for initialization
echo "[INFO] Waiting for gNodeB initialization..."
sleep 5

# Check if running
if docker exec albor-gnb-dev pgrep gnb > /dev/null; then
    echo "[INFO] ✓ gNodeB is running in no-core mode"
    
    # Check logs for no-core confirmation
    docker exec albor-gnb-dev grep -i "no.core\|without.core\|stub" /tmp/gnb_stdout.log 2>/dev/null || echo "[INFO] No explicit no-core message found"
else
    echo "[ERROR] gNodeB failed to start"
    docker exec albor-gnb-dev tail -20 /tmp/gnb_stdout.log
    exit 1
fi

# Start UE
echo "[INFO] Starting srsUE..."
docker exec -d albor-gnb-dev bash -c "
    cd /opt/srsran
    export LD_LIBRARY_PATH=/opt/srsran/lib:\$LD_LIBRARY_PATH
    /opt/srsran/bin/srsue /workspace/config/srsue/ue_nr_zmq_10mhz.conf > /tmp/ue.log 2>&1
"

# Monitor for 20 seconds
echo "[INFO] Monitoring for RRC connection..."
for i in {1..20}; do
    # Check for cell detection
    if docker exec albor-gnb-dev grep -q "Found Cell" /tmp/ue.log 2>/dev/null; then
        echo "[INFO] ✓ Cell detected"
        
        # Check for RRC
        if docker exec albor-gnb-dev grep -q "RRC Connected" /tmp/ue.log 2>/dev/null; then
            echo "[INFO] ✓ RRC Connected!"
            break
        fi
    fi
    
    printf "\r[%02d/20] Waiting..." "$i"
    sleep 1
done
echo ""

# Copy logs
echo "[INFO] Copying logs..."
docker exec albor-gnb-dev cat /tmp/gnb_stdout.log > "$LOG_DIR/gnb_stdout.log" 2>/dev/null || true
docker exec albor-gnb-dev cat /tmp/gnb.log > "$LOG_DIR/gnb.log" 2>/dev/null || true
docker exec albor-gnb-dev cat /tmp/ue.log > "$LOG_DIR/ue.log" 2>/dev/null || true

# Show results
echo ""
echo "=== Results ==="
echo "gNodeB status:"
docker exec albor-gnb-dev tail -10 /tmp/gnb_stdout.log 2>/dev/null | grep -v "^$" || echo "No gNodeB log"

echo ""
echo "UE status:"
docker exec albor-gnb-dev grep -E "Cell|RRC|Random Access" /tmp/ue.log 2>/dev/null | tail -5 || echo "No UE milestones"

# Cleanup
echo ""
echo "[INFO] Cleaning up..."
docker exec albor-gnb-dev bash -c "pkill -9 gnb 2>/dev/null || true; pkill -9 srsue 2>/dev/null || true"

echo "[INFO] Test complete. Logs saved to $LOG_DIR"