#!/bin/bash
# test_zmq_comparison.sh - Test with consistent sample rates
# This script tests both configurations to find what works

set -e

CONTAINER_NAME="albor-gnb-dev"
LOG_DIR="./logs/$(date +%Y%m%d_%H%M%S)_zmq_comparison"

mkdir -p "$LOG_DIR"

echo "[$(date +%H:%M:%S)] ZMQ Sample Rate Comparison Test"
echo "[$(date +%H:%M:%S)] Log directory: $LOG_DIR"

# Clean up
echo "[$(date +%H:%M:%S)] Cleaning up previous runs..."
docker exec $CONTAINER_NAME bash -c "pkill -9 albor_gnodeb 2>/dev/null || true"
docker exec $CONTAINER_NAME bash -c "pkill -9 srsue 2>/dev/null || true"
sleep 1

# Build
echo "[$(date +%H:%M:%S)] Compiling Albor gNodeB..."
if ! docker exec $CONTAINER_NAME cargo build 2>&1 | tee "$LOG_DIR/build.log"; then
    echo "[ERROR] Build failed"
    exit 1
fi

# Test 1: Both at 11.52 MHz (config file rate)
echo ""
echo "================================================"
echo "[$(date +%H:%M:%S)] TEST 1: Both at 11.52 MHz"
echo "================================================"

# Start gNodeB
echo "[$(date +%H:%M:%S)] Starting Albor gNodeB (11.52 MHz)..."
docker exec -d $CONTAINER_NAME bash -c "/workspace/target/debug/albor_gnodeb -c /workspace/config/albor_gnb/gnb_albor.yml > /tmp/gnb_11mhz.log 2>&1"

sleep 5

# Start UE at 11.52 MHz (default from config)
echo "[$(date +%H:%M:%S)] Starting srsUE (11.52 MHz from config)..."
docker exec -d $CONTAINER_NAME bash -c "export LD_LIBRARY_PATH=/opt/srsran/lib:\$LD_LIBRARY_PATH && /opt/srsran/bin/srsue /workspace/config/srsue/ue_nr_zmq_10mhz.conf --rat.nr.dl_nr_arfcn 368500 --rat.nr.ssb_nr_arfcn 368410 --rat.nr.nof_prb 52 --rat.nr.scs 15 --rat.nr.ssb_scs 15 > /tmp/ue_11mhz.log 2>&1"

# Monitor for 10 seconds
echo "[$(date +%H:%M:%S)] Monitoring for 10 seconds..."
for i in {1..10}; do
    sleep 1
    echo -n "."
    if docker exec $CONTAINER_NAME bash -c "grep -q 'Found Cell' /tmp/ue_11mhz.log 2>/dev/null"; then
        echo ""
        echo "[$(date +%H:%M:%S)] SUCCESS: UE found cell at 11.52 MHz!"
        break
    fi
done
echo ""

# Copy logs
docker cp $CONTAINER_NAME:/tmp/gnb_11mhz.log "$LOG_DIR/" 2>/dev/null || true
docker cp $CONTAINER_NAME:/tmp/ue_11mhz.log "$LOG_DIR/" 2>/dev/null || true

# Check results
echo "[$(date +%H:%M:%S)] Test 1 Results:"
if docker exec $CONTAINER_NAME bash -c "grep -q 'Found Cell' /tmp/ue_11mhz.log 2>/dev/null"; then
    echo "✓ Cell detected at 11.52 MHz"
else
    echo "✗ Cell NOT detected at 11.52 MHz"
    echo "UE status:"
    docker exec $CONTAINER_NAME tail -5 /tmp/ue_11mhz.log 2>/dev/null || echo "No log"
fi

# Kill processes
docker exec $CONTAINER_NAME bash -c "pkill -9 albor_gnodeb 2>/dev/null || true"
docker exec $CONTAINER_NAME bash -c "pkill -9 srsue 2>/dev/null || true"
sleep 2

# Test 2: Both at 15.36 MHz (PHY natural rate)
echo ""
echo "================================================"
echo "[$(date +%H:%M:%S)] TEST 2: Both at 15.36 MHz"
echo "================================================"

# Start gNodeB (still generates at 15.36 MHz internally)
echo "[$(date +%H:%M:%S)] Starting Albor gNodeB (generates at 15.36 MHz)..."
docker exec -d $CONTAINER_NAME bash -c "/workspace/target/debug/albor_gnodeb -c /workspace/config/albor_gnb/gnb_albor.yml > /tmp/gnb_15mhz.log 2>&1"

sleep 5

# Start UE at 15.36 MHz (override via device args)
echo "[$(date +%H:%M:%S)] Starting srsUE (15.36 MHz override)..."
docker exec -d $CONTAINER_NAME bash -c "export LD_LIBRARY_PATH=/opt/srsran/lib:\$LD_LIBRARY_PATH && /opt/srsran/bin/srsue /workspace/config/srsue/ue_nr_zmq_10mhz.conf --rf.device_args='tx_port=tcp://127.0.0.1:2001,rx_port=tcp://127.0.0.1:2000,base_srate=15.36e6' --rat.nr.dl_nr_arfcn 368500 --rat.nr.ssb_nr_arfcn 368410 --rat.nr.nof_prb 52 --rat.nr.scs 15 --rat.nr.ssb_scs 15 > /tmp/ue_15mhz.log 2>&1"

# Monitor for 10 seconds
echo "[$(date +%H:%M:%S)] Monitoring for 10 seconds..."
for i in {1..10}; do
    sleep 1
    echo -n "."
    if docker exec $CONTAINER_NAME bash -c "grep -q 'Found Cell' /tmp/ue_15mhz.log 2>/dev/null"; then
        echo ""
        echo "[$(date +%H:%M:%S)] SUCCESS: UE found cell at 15.36 MHz!"
        break
    fi
done
echo ""

# Copy logs
docker cp $CONTAINER_NAME:/tmp/gnb_15mhz.log "$LOG_DIR/" 2>/dev/null || true
docker cp $CONTAINER_NAME:/tmp/ue_15mhz.log "$LOG_DIR/" 2>/dev/null || true

# Check results
echo "[$(date +%H:%M:%S)] Test 2 Results:"
if docker exec $CONTAINER_NAME bash -c "grep -q 'Found Cell' /tmp/ue_15mhz.log 2>/dev/null"; then
    echo "✓ Cell detected at 15.36 MHz"
else
    echo "✗ Cell NOT detected at 15.36 MHz"
    echo "UE status:"
    docker exec $CONTAINER_NAME tail -5 /tmp/ue_15mhz.log 2>/dev/null || echo "No log"
fi

# Kill processes
docker exec $CONTAINER_NAME bash -c "pkill -9 albor_gnodeb 2>/dev/null || true"
docker exec $CONTAINER_NAME bash -c "pkill -9 srsue 2>/dev/null || true"

echo ""
echo "================================================"
echo "[$(date +%H:%M:%S)] SUMMARY"
echo "================================================"
echo "Test 1 (11.52 MHz): Check $LOG_DIR/ue_11mhz.log"
echo "Test 2 (15.36 MHz): Check $LOG_DIR/ue_15mhz.log"
echo ""
echo "Key finding: The gNodeB generates at 15.36 MHz internally"
echo "but the resampler to 11.52 MHz is NOT being applied!"
echo "This causes a sample rate mismatch."