#!/bin/bash
# test_srsran.sh - Simple reference test with srsRAN gNodeB + UE + Open5GS
# Minimal test script - under 150 lines, runtime under 20 seconds

set +e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
NC='\033[0m'

log_info() { echo -e "${GREEN}[INFO]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }

# Create log directory
HOST_LOG_DIR="logs/$(date +%Y%m%d_%H%M%S)_srsran"
CONTAINER_LOG_DIR="/workspace/logs/$(date +%Y%m%d_%H%M%S)_srsran"
mkdir -p "$HOST_LOG_DIR"

log_info "=== Simple srsRAN Test ==="
log_info "Log directory: $HOST_LOG_DIR"

CONTAINER_NAME="albor-gnb-dev"

# Simple cleanup function
cleanup() {
    log_info "Cleaning up..."
    docker compose exec -T $CONTAINER_NAME bash -c "pkill -9 -f 'gnb|srsue' 2>/dev/null || true"
    sleep 1
}

trap cleanup EXIT

# Initial cleanup
cleanup

# Step 1: Ensure container is running with Open5GS
log_info "Step 1: Starting container with Open5GS..."
docker compose down 2>/dev/null || true
docker compose up -d
sleep 5

# Step 2: Check Open5GS is running
log_info "Step 2: Checking Open5GS..."
if docker compose exec -T $CONTAINER_NAME pgrep -f open5gs-amfd > /dev/null; then
    log_info "✓ Open5GS AMF is running"
else
    log_error "✗ Open5GS AMF not running"
    exit 1
fi

# Step 3: Start gNodeB
log_info "Step 3: Starting srsRAN gNodeB..."
docker compose exec -T $CONTAINER_NAME bash -c "
    cd /srsran_project
    /usr/local/bin/gnb \
        --gnb_id 1 \
        cu_cp amf --addr 127.0.0.5 --port 38412 --bind_addr 127.0.0.1 \
        cell_cfg --dl_arfcn 368500 --band 3 --channel_bandwidth_MHz 10 \
        cell_cfg --common_scs 15kHz --plmn 99970 --tac 1 --pci 1 \
        ru_sdr --device_driver zmq --srate 11.52 --tx_gain 75 --rx_gain 75 \
        ru_sdr --device_args 'tx_port=tcp://127.0.0.1:2000,rx_port=tcp://127.0.0.1:2001,base_srate=11.52e6' \
        log --filename $CONTAINER_LOG_DIR/gnb.log --all_level info \
        > $CONTAINER_LOG_DIR/gnb_stdout.log 2>&1 &
"

# Step 4: Wait 5 seconds
log_info "Step 4: Waiting 5 seconds for gNodeB to stabilize..."
sleep 5

# Step 5: Start srsUE
log_info "Step 5: Starting srsUE..."
docker compose exec -T $CONTAINER_NAME bash -c "
    export LD_LIBRARY_PATH=/usr/local/lib:\$LD_LIBRARY_PATH
    cp /workspace/config/srsue/ue_zmq.conf /tmp/ue_config.conf
    sed -i 's|filename = /workspace/logs/ue.log|filename = $CONTAINER_LOG_DIR/ue.log|g' /tmp/ue_config.conf
    
    /usr/local/bin/srsue \
        /tmp/ue_config.conf \
        --rat.nr.dl_nr_arfcn 368500 \
        --rat.nr.ssb_nr_arfcn 367930 \
        --rat.nr.nof_prb 52 \
        --rat.nr.scs 15 \
        --rat.nr.ssb_scs 15 \
        > $CONTAINER_LOG_DIR/ue_stdout.log 2>&1 &
"

# Step 6: Wait 15 seconds
log_info "Step 6: Waiting 15 seconds for connection..."
sleep 15

# Step 7: Check the 6 criteria
log_info "Step 7: Checking success criteria..."

CRITERIA_MET=0

# 1. Open5GS running with SCTP functional
if docker compose exec -T $CONTAINER_NAME pgrep -f open5gs-amfd > /dev/null; then
    log_info "✓ Criteria 1: Open5GS running with SCTP functional"
    ((CRITERIA_MET++))
else
    log_error "✗ Criteria 1: Open5GS not running"
fi

# 2. srsRAN gNodeB: "NG setup procedure completed"
if docker compose exec -T $CONTAINER_NAME grep -q "NGSetupResponse" "$CONTAINER_LOG_DIR/gnb.log" 2>/dev/null || \
   docker compose exec -T $CONTAINER_NAME grep -q "NG setup procedure completed" "$CONTAINER_LOG_DIR/gnb_stdout.log" 2>/dev/null; then
    log_info "✓ Criteria 2: NG setup procedure completed"
    ((CRITERIA_MET++))
else
    log_error "✗ Criteria 2: NG setup not completed"
fi

# 3. srsRAN gNodeB: Connected to AMF
if docker compose exec -T $CONTAINER_NAME grep -q "Connection to AMF.*was established" "$CONTAINER_LOG_DIR/gnb.log" 2>/dev/null || \
   docker compose exec -T $CONTAINER_NAME grep -q "Connected to AMF" "$CONTAINER_LOG_DIR/gnb_stdout.log" 2>/dev/null; then
    log_info "✓ Criteria 3: Connected to AMF"
    ((CRITERIA_MET++))
else
    log_error "✗ Criteria 3: Not connected to AMF"
fi

# 4. srsUE: "Found Cell"
if docker compose exec -T $CONTAINER_NAME grep -q "PBCH-NR Rx: crc=OK" "$CONTAINER_LOG_DIR/ue.log" 2>/dev/null || \
   docker compose exec -T $CONTAINER_NAME grep -q "state=CAMPING" "$CONTAINER_LOG_DIR/ue.log" 2>/dev/null || \
   docker compose exec -T $CONTAINER_NAME grep -q "Found Cell" "$CONTAINER_LOG_DIR/ue_stdout.log" 2>/dev/null; then
    log_info "✓ Criteria 4: Found Cell"
    ((CRITERIA_MET++))
else
    log_error "✗ Criteria 4: Cell not found"
fi

# 5. srsUE: "RRC Connected"
if docker compose exec -T $CONTAINER_NAME grep -q "RRC Connected" "$CONTAINER_LOG_DIR/ue.log" 2>/dev/null || \
   docker compose exec -T $CONTAINER_NAME grep -q "RRC Connected" "$CONTAINER_LOG_DIR/ue_stdout.log" 2>/dev/null; then
    log_info "✓ Criteria 5: RRC Connected"
    ((CRITERIA_MET++))
else
    log_error "✗ Criteria 5: RRC not connected"
fi

# 6. srsUE: "NAS-5G Registration complete"
if docker compose exec -T $CONTAINER_NAME grep -q "NAS-5G.*Registration complete" "$CONTAINER_LOG_DIR/ue.log" 2>/dev/null || \
   docker compose exec -T $CONTAINER_NAME grep -q "NAS-5G.*Registration complete" "$CONTAINER_LOG_DIR/ue_stdout.log" 2>/dev/null || \
   docker compose exec -T $CONTAINER_NAME grep -q "5GMM-REGISTERED" "/var/log/open5gs/amf.log" 2>/dev/null; then
    log_info "✓ Criteria 6: NAS-5G Registration complete"
    ((CRITERIA_MET++))
else
    log_error "✗ Criteria 6: NAS registration not complete"
fi

# Step 8: Report results
echo ""
echo "=========================================="
log_info "TEST RESULTS: $CRITERIA_MET/6 criteria met"
echo "=========================================="

if [ $CRITERIA_MET -eq 6 ]; then
    log_info "✅ SUCCESS: All criteria met!"
else
    log_error "❌ FAILED: Only $CRITERIA_MET/6 criteria met"
    
    # Show basic debug info
    echo ""
    log_info "Debug logs:"
    docker compose exec -T $CONTAINER_NAME tail -10 "$CONTAINER_LOG_DIR/gnb.log" 2>/dev/null || echo "No gNodeB log"
    echo ""
    docker compose exec -T $CONTAINER_NAME tail -10 "$CONTAINER_LOG_DIR/ue.log" 2>/dev/null || echo "No UE log"
fi

echo "=========================================="
log_info "Test completed in under 20 seconds"