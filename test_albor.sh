#!/bin/bash
# test_albor.sh - 10-second test for Albor gNodeB + srsUE
# Run from HOST machine - all operations via docker exec
# Shows logs in real-time and kills processes properly
# Usage: ./test_albor.sh [--debug|-d] to use debug binary

set -e

# Parse command line arguments
BUILD_TYPE="release"
BINARY_PATH="/workspace/target/release/albor_gnodeb"
if [[ "$1" == "--debug" ]] || [[ "$1" == "-d" ]]; then
    BUILD_TYPE="debug"
    BINARY_PATH="/workspace/target/debug/albor_gnodeb"
fi

# Configuration
CONTAINER_NAME="albor-gnb-dev"
LOG_DIR="./logs/$(date +%Y%m%d_%H%M%S)_albor_${BUILD_TYPE}"

# Create log directory on host
mkdir -p "$LOG_DIR"

echo "[$(date +%H:%M:%S)] Starting Albor test (10 second duration)"
echo "[$(date +%H:%M:%S)] Log directory: $LOG_DIR"

# Step 1: Clean up any existing processes
echo "[$(date +%H:%M:%S)] Cleaning up previous runs..."
# Use simpler pkill commands to avoid hanging
docker compose exec -T $CONTAINER_NAME bash -c "pkill -9 albor_gnodeb 2>/dev/null || true"
docker compose exec -T $CONTAINER_NAME bash -c "pkill -9 srsue 2>/dev/null || true"
# Clean up any stray processes
docker compose exec -T $CONTAINER_NAME bash -c "rm -f /tmp/ue.log /tmp/gnb.log 2>/dev/null || true"
sleep 1

# Step 2: Ensure Docker container is running
echo "[$(date +%H:%M:%S)] Ensuring Docker container is running..."
docker compose up -d
sleep 5

# Step 2.5: Compile with cargo build if needed
echo "[$(date +%H:%M:%S)] Checking if compilation is needed..."
if ! docker compose exec -T $CONTAINER_NAME test -f "$BINARY_PATH"; then
    echo "[$(date +%H:%M:%S)] Binary not found. Compiling Albor in $BUILD_TYPE mode..."
    docker compose exec -T $CONTAINER_NAME bash -c "cd /workspace && cargo build $([ "$BUILD_TYPE" = "release" ] && echo "--release" || echo "")"
else
    echo "[$(date +%H:%M:%S)] Using pre-compiled $BUILD_TYPE binary..."
fi

# Step 3: Start gNodeB in background and capture logs
echo "[$(date +%H:%M:%S)] Starting Albor gNodeB..."
docker compose exec -d $CONTAINER_NAME bash -c "$BINARY_PATH -c /workspace/config/albor_gnb/gnb_albor.yml > /tmp/gnb.log 2>&1"

# Wait 7 seconds for gNodeB to initialize and pre-buffer samples
echo "[$(date +%H:%M:%S)] Waiting 7 seconds for gNodeB initialization and pre-buffering..."
sleep 7

# Step 4: Start UE in background
echo "[$(date +%H:%M:%S)] Starting srsUE..."
# Start UE with our ZMQ config
docker compose exec -d $CONTAINER_NAME bash -c "export LD_LIBRARY_PATH=/usr/local/lib:\$LD_LIBRARY_PATH && tail -f /dev/null | /usr/local/bin/srsue /workspace/config/srsue/ue_zmq.conf --rat.nr.dl_nr_arfcn 368500 --rat.nr.ssb_nr_arfcn 368410 --rat.nr.nof_prb 52 --rat.nr.scs 15 --rat.nr.ssb_scs 15 --log.phy_level=info --log.phy_lib_level=info > /tmp/ue.log 2>&1"

# Step 5: Monitor for 8 seconds, showing log progress
echo "[$(date +%H:%M:%S)] Running test for 8 seconds..."
for i in {1..8}; do
    sleep 1
    echo -n "[$i] "
    
    # Show gNodeB progress
    gnb_lines=$(docker compose exec -T $CONTAINER_NAME bash -c "wc -l < /tmp/gnb.log 2>/dev/null || echo 0")
    echo -n "gNodeB: $gnb_lines lines "
    
    # Show UE progress
    ue_lines=$(docker compose exec -T $CONTAINER_NAME bash -c "wc -l < /tmp/ue.log 2>/dev/null || echo 0")
    echo "| UE: $ue_lines lines"
    
    # Check for cell detection
    if docker compose exec -T $CONTAINER_NAME bash -c "grep -q 'Found Cell' /tmp/ue.log 2>/dev/null"; then
        echo "[$(date +%H:%M:%S)] SUCCESS: UE found cell!"
    fi
done

# Step 6: Kill processes
echo "[$(date +%H:%M:%S)] Stopping processes..."
docker compose exec -T $CONTAINER_NAME bash -c "pkill -TERM albor_gnodeb 2>/dev/null || true"
docker compose exec -T $CONTAINER_NAME bash -c "pkill -TERM srsue 2>/dev/null || true"
sleep 1
docker compose exec -T $CONTAINER_NAME bash -c "pkill -9 albor_gnodeb 2>/dev/null || true"
docker compose exec -T $CONTAINER_NAME bash -c "pkill -9 srsue 2>/dev/null || true"

# Step 7: Copy logs to host
echo "[$(date +%H:%M:%S)] Copying logs to host..."
docker cp $CONTAINER_NAME:/tmp/gnb.log "$LOG_DIR/gnb.log" 2>/dev/null || echo "[WARN] No gNodeB log"
docker cp $CONTAINER_NAME:/tmp/ue.log "$LOG_DIR/ue.log" 2>/dev/null || echo "[WARN] No UE log"

# Copy PCAP files if they exist
for pcap in gnb_mac.pcap gnb_ngap.pcap ue_mac.pcap ue_mac_nr.pcap ue_nas.pcap; do
    docker cp $CONTAINER_NAME:/tmp/$pcap "$LOG_DIR/$pcap" 2>/dev/null || true
done

# Step 8: Show results
echo ""
echo "[$(date +%H:%M:%S)] Test complete! Analyzing results..."
echo "==============================================="

# Show gNodeB results
if [ -f "$LOG_DIR/gnb.log" ]; then
    gnb_lines=$(wc -l < "$LOG_DIR/gnb.log")
    echo "gNodeB log: $gnb_lines lines"
    
    # Check for key events
    if grep -q "SSB transmission period starting" "$LOG_DIR/gnb.log"; then
        echo "✓ SSB transmission started"
    else
        echo "✗ SSB transmission NOT started"
    fi
    
    if grep -q "PRACH detection" "$LOG_DIR/gnb.log"; then
        echo "✓ PRACH detection active"
    fi
    
    echo ""
    echo "Last 10 lines of gNodeB log:"
    echo "----------------------------"
    tail -10 "$LOG_DIR/gnb.log"
else
    echo "✗ No gNodeB log generated!"
fi

echo ""
echo "==============================================="

# Show UE results
if [ -f "$LOG_DIR/ue.log" ]; then
    ue_lines=$(wc -l < "$LOG_DIR/ue.log")
    echo "UE log: $ue_lines lines"
    
    # Check for cell detection
    if grep -q "Found Cell" "$LOG_DIR/ue.log"; then
        echo "✓ Cell FOUND!"
        grep "Found Cell" "$LOG_DIR/ue.log" | tail -1
    else
        echo "✗ Cell NOT found"
    fi
    
    # Check for RRC connection
    if grep -q "RRC Connected" "$LOG_DIR/ue.log"; then
        echo "✓ RRC Connected!"
    fi
    
    echo ""
    echo "Last 10 lines of UE log:"
    echo "------------------------"
    tail -10 "$LOG_DIR/ue.log"
else
    echo "✗ No UE log generated!"
fi

echo ""
echo "==============================================="
echo "Full logs saved to: $LOG_DIR"
echo ""

# Check for errors
if grep -q "ERROR\|FATAL\|Failed" "$LOG_DIR/gnb.log" 2>/dev/null; then
    echo "[WARN] Errors found in gNodeB log:"
    grep -E "ERROR|FATAL|Failed" "$LOG_DIR/gnb.log" | tail -5
fi

# Final cleanup
docker compose exec -T $CONTAINER_NAME rm -f /tmp/gnb.log /tmp/ue.log 2>/dev/null || true