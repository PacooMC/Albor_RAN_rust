#!/bin/bash
# test_srsran_phy_only.sh - Test srsRAN gNodeB PHY layer without core network
# This tests if srsRAN can transmit SSBs that srsUE can detect

set -e

# Configuration
CONTAINER_NAME="albor-gnb-dev"
LOG_DIR="./logs/$(date +%Y%m%d_%H%M%S)_srsran_phy"

# Create log directory
mkdir -p "$LOG_DIR"

echo "[$(date +%H:%M:%S)] Starting srsRAN PHY-only test"
echo "[$(date +%H:%M:%S)] Log directory: $LOG_DIR"

# Step 1: Clean up
echo "[$(date +%H:%M:%S)] Cleaning up previous runs..."
docker exec $CONTAINER_NAME bash -c "pkill -9 gnb 2>/dev/null || true"
docker exec $CONTAINER_NAME bash -c "pkill -9 srsue 2>/dev/null || true"
sleep 1

# Step 2: Start srsRAN gNodeB with sacred config but no AMF
echo "[$(date +%H:%M:%S)] Starting srsRAN gNodeB..."

# Copy sacred config and modify to remove AMF
docker exec $CONTAINER_NAME bash -c "cp /workspace/config/srsran_gnb/gnb_zmq_10mhz.yml /tmp/gnb_no_amf.yml"

# Remove AMF config line (comment it out)
docker exec $CONTAINER_NAME bash -c "sed -i 's/^amf:/#amf:/' /tmp/gnb_no_amf.yml"
docker exec $CONTAINER_NAME bash -c "sed -i 's/^  addr:/#  addr:/' /tmp/gnb_no_amf.yml"
docker exec $CONTAINER_NAME bash -c "sed -i 's/^  bind_addr:/#  bind_addr:/' /tmp/gnb_no_amf.yml"

# Start with modified config
docker exec -d $CONTAINER_NAME bash -c "cd /opt/srsran_project && ./bin/gnb -c /tmp/gnb_no_amf.yml > /tmp/srsran_gnb.log 2>&1"

# Wait for initialization
echo "[$(date +%H:%M:%S)] Waiting 5 seconds for gNodeB initialization..."
sleep 5

# Step 3: Start srsUE
echo "[$(date +%H:%M:%S)] Starting srsUE..."
docker exec -d $CONTAINER_NAME bash -c "cd /opt/srsran && export LD_LIBRARY_PATH=/opt/srsran/lib:\$LD_LIBRARY_PATH && tail -f /dev/null | /opt/srsran/bin/srsue /workspace/config/srsue/ue_nr_zmq_10mhz.conf --rat.nr.dl_nr_arfcn 368500 --rat.nr.ssb_nr_arfcn 368410 --rat.nr.nof_prb 52 --rat.nr.scs 15 --rat.nr.ssb_scs 15 --log.phy_level=info > /tmp/ue.log 2>&1"

# Step 4: Monitor for 10 seconds
echo "[$(date +%H:%M:%S)] Running test for 10 seconds..."
for i in {1..10}; do
    sleep 1
    echo -n "[$i] "
    
    # Check for cell detection
    if docker exec $CONTAINER_NAME bash -c "grep -q 'Found Cell' /tmp/ue.log 2>/dev/null"; then
        echo ""
        echo "[$(date +%H:%M:%S)] SUCCESS: UE found srsRAN cell!"
        docker exec $CONTAINER_NAME bash -c "grep 'Found Cell' /tmp/ue.log | tail -1"
        break
    fi
done
echo ""

# Step 5: Stop processes
echo "[$(date +%H:%M:%S)] Stopping processes..."
docker exec $CONTAINER_NAME bash -c "pkill -TERM gnb 2>/dev/null || true"
docker exec $CONTAINER_NAME bash -c "pkill -TERM srsue 2>/dev/null || true"
sleep 1

# Step 6: Copy logs
echo "[$(date +%H:%M:%S)] Copying logs to host..."
docker cp $CONTAINER_NAME:/tmp/srsran_gnb.log "$LOG_DIR/gnb.log" 2>/dev/null || echo "[WARN] No gNodeB log"
docker cp $CONTAINER_NAME:/tmp/ue.log "$LOG_DIR/ue.log" 2>/dev/null || echo "[WARN] No UE log"

# Step 7: Show results
echo ""
echo "[$(date +%H:%M:%S)] Test complete! Results:"
echo "======================================="

if [ -f "$LOG_DIR/ue.log" ]; then
    if grep -q "Found Cell" "$LOG_DIR/ue.log"; then
        echo "✓ Cell DETECTED by srsUE!"
        grep "Found Cell" "$LOG_DIR/ue.log" | tail -3
    else
        echo "✗ Cell NOT detected"
    fi
    
    echo ""
    echo "UE log (last 10 lines):"
    tail -10 "$LOG_DIR/ue.log"
else
    echo "✗ No UE log generated!"
fi

echo ""
echo "======================================="
echo "Full logs saved to: $LOG_DIR"

# Cleanup
docker exec $CONTAINER_NAME rm -f /tmp/gnb_minimal.yml /tmp/srsran_gnb.log /tmp/ue.log 2>/dev/null || true