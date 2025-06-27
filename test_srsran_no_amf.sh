#!/bin/bash
# test_srsran_no_amf.sh - Test srsRAN gNodeB + UE without AMF
# This tests if srsUE can detect srsRAN gNodeB cell even without core network

set -e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

log_info() { echo -e "${GREEN}[INFO]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }
log_warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }

# Create log directory
LOG_DIR="logs/$(date +%Y%m%d_%H%M%S)_srsran_no_amf"
mkdir -p "$LOG_DIR"

log_info "=== srsRAN Cell Detection Test (No AMF) ==="
log_info "Testing: srsRAN gNodeB + srsRAN UE (without core network)"
log_info "Log directory: $LOG_DIR"
DOCKER_LOG_DIR="/workspace/$LOG_DIR"

# Cleanup function
cleanup() {
    log_info "Cleaning up..."
    docker exec albor-gnb-dev bash -c "pkill -9 -f 'gnb|srsue' 2>/dev/null || true"
    sleep 2
}

trap cleanup EXIT

# Initial cleanup
cleanup

# Step 1: Start srsRAN gNodeB without AMF connection
log_info "Starting srsRAN gNodeB (no AMF mode)..."

# Modified gNodeB command - remove AMF connection parameters
docker exec albor-gnb-dev bash -c "
    cd /opt/srsran_project
    /opt/srsran_project/bin/gnb \
        --gnb_id 1 \
        cell_cfg --dl_arfcn 368500 --band 3 --channel_bandwidth_MHz 10 \
        cell_cfg --common_scs 15kHz --plmn 00101 --tac 7 --pci 1 \
        ru_sdr --device_driver zmq --srate 11.52 --tx_gain 75 --rx_gain 75 \
        ru_sdr --device_args 'tx_port=tcp://127.0.0.1:2000,rx_port=tcp://127.0.0.1:2001,base_srate=11.52e6' \
        log --filename $DOCKER_LOG_DIR/gnb.log --all_level info \
        > $DOCKER_LOG_DIR/gnb_stdout.log 2>&1 &
"

log_info "Waiting for gNodeB to initialize..."
sleep 5

# Check if gNodeB started
if docker exec albor-gnb-dev pgrep -f gnb > /dev/null; then
    log_info "✓ gNodeB process started"
else
    log_error "✗ gNodeB failed to start"
    docker exec albor-gnb-dev tail -50 "$DOCKER_LOG_DIR/gnb_stdout.log" 2>/dev/null || echo "No log available"
    exit 1
fi

# Step 2: Start srsUE
log_info "Starting srsUE..."

docker exec albor-gnb-dev bash -c "
    cd /opt/srsran
    export LD_LIBRARY_PATH=/opt/srsran/lib:\$LD_LIBRARY_PATH
    
    # Create temporary config with updated log paths
    cp /workspace/config/srsue/ue_nr_zmq_10mhz.conf /tmp/ue_config.conf
    sed -i 's|filename = /tmp/ue.log|filename = $DOCKER_LOG_DIR/ue.log|g' /tmp/ue_config.conf
    
    /opt/srsran/bin/srsue \
        /tmp/ue_config.conf \
        --rat.nr.dl_nr_arfcn 368500 \
        --rat.nr.ssb_nr_arfcn 368410 \
        --rat.nr.nof_prb 52 \
        --rat.nr.scs 15 \
        --rat.nr.ssb_scs 15 \
        > $DOCKER_LOG_DIR/ue_stdout.log 2>&1 &
"

log_info "Waiting for UE to start..."
sleep 3

# Step 3: Monitor for cell detection
log_info "Monitoring for cell detection..."

TIMEOUT=30
CELL_FOUND=false

for i in $(seq 1 $TIMEOUT); do
    printf "\r[%02d/%02d] Checking for cell detection..." "$i" "$TIMEOUT"
    
    # Check for cell detection in UE logs
    if docker exec albor-gnb-dev grep -q "Found Cell.*PCI=1" "$DOCKER_LOG_DIR/ue.log" 2>/dev/null || \
       docker exec albor-gnb-dev grep -q "Found Cell.*PCI=1" "$DOCKER_LOG_DIR/ue_stdout.log" 2>/dev/null; then
        CELL_FOUND=true
        echo ""
        log_info "✓ UE found cell (PCI=1)!"
        break
    fi
    
    sleep 1
done

echo ""
echo "=========================================="
log_info "TEST RESULTS:"
echo "=========================================="

if [ "$CELL_FOUND" = "true" ]; then
    log_info "✅ SUCCESS: srsUE detected srsRAN gNodeB cell!"
    
    # Show cell detection details
    log_info "Cell detection logs:"
    docker exec albor-gnb-dev bash -c "
        grep -E 'Found Cell|cell_search|PLMN|PCI' '$DOCKER_LOG_DIR/ue.log' 2>/dev/null | tail -10 || \
        grep -E 'Found Cell|cell_search|PLMN|PCI' '$DOCKER_LOG_DIR/ue_stdout.log' 2>/dev/null | tail -10
    "
else
    log_error "❌ FAILED: srsUE did not detect cell within ${TIMEOUT} seconds"
    
    # Debug information
    log_info "gNodeB log tail:"
    docker exec albor-gnb-dev tail -30 "$DOCKER_LOG_DIR/gnb_stdout.log" 2>/dev/null || echo "No log"
    
    echo ""
    log_info "UE log tail:"
    docker exec albor-gnb-dev tail -30 "$DOCKER_LOG_DIR/ue_stdout.log" 2>/dev/null || echo "No log"
fi

echo "=========================================="

# Keep running for a bit to observe behavior
if [ "$CELL_FOUND" = "true" ]; then
    log_info "Monitoring for 10 more seconds..."
    sleep 10
    
    # Check what happens after cell detection
    log_info "Post-detection behavior:"
    docker exec albor-gnb-dev tail -20 "$DOCKER_LOG_DIR/ue.log" 2>/dev/null || \
    docker exec albor-gnb-dev tail -20 "$DOCKER_LOG_DIR/ue_stdout.log" 2>/dev/null
fi