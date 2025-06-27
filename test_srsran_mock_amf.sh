#!/bin/bash
# test_srsran_mock_amf.sh - Test srsRAN with Mock AMF (No SCTP Required)
# This script uses a TCP-based mock AMF to bypass SCTP kernel module requirements
# Allows testing in Docker without --privileged flag

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
log_debug() { echo -e "${BLUE}[DEBUG]${NC} $1"; }

# Create log directory
LOG_DIR="./logs/$(date +%Y%m%d_%H%M%S)_srsran_mock"
mkdir -p "$LOG_DIR"

log_info "=== srsRAN with Mock AMF Test ==="
log_info "Using TCP-based Mock AMF (No SCTP required)"
log_info "Log directory: $LOG_DIR"

# Cleanup function
cleanup() {
    log_info "Cleaning up processes..."
    pkill -9 -f "mock_amf.py" 2>/dev/null || true
    pkill -9 -f "gnb" 2>/dev/null || true
    pkill -9 -f "srsue" 2>/dev/null || true
    sleep 1
}

# Set trap for cleanup
trap cleanup EXIT

# Initial cleanup
cleanup

# Step 1: Start Mock AMF
log_info "Step 1: Starting Mock AMF on TCP port 38412..."

# Start the mock AMF
python3 ./scripts/mock_amf.py 127.0.0.4 38412 > "$LOG_DIR/mock_amf.log" 2>&1 &
MOCK_AMF_PID=$!

# Wait for mock AMF to start
sleep 2

# Check if mock AMF is running
if ! ps -p $MOCK_AMF_PID > /dev/null; then
    log_error "Failed to start Mock AMF"
    cat "$LOG_DIR/mock_amf.log"
    exit 1
fi

# Check if port is listening
if netstat -tuln 2>/dev/null | grep -q "127.0.0.4:38412"; then
    log_info "✓ Mock AMF listening on 127.0.0.4:38412 (TCP)"
else
    log_error "Mock AMF not listening on expected port"
    exit 1
fi

# Step 2: Configure and start srsRAN gNodeB
log_info "Step 2: Starting srsRAN gNodeB..."

# Since srsRAN expects SCTP, we need to handle the connection failure gracefully
# The gNodeB will try SCTP first, fail, but we can still test PHY layer

cd /opt/srsran_project

# Start gNodeB with command line parameters
/opt/srsran_project/bin/gnb \
    --gnb_id 1 \
    cu_cp amf --addr 127.0.0.4 --port 38412 --bind_addr 127.0.0.11 \
    cell_cfg --dl_arfcn 368500 --band 3 --channel_bandwidth_MHz 10 \
    cell_cfg --common_scs 15kHz --plmn 00101 --tac 7 --pci 1 \
    ru_sdr --device_driver zmq --srate 11.52 --tx_gain 75 --rx_gain 75 \
    ru_sdr --device_args "tx_port=tcp://127.0.0.1:2000,rx_port=tcp://127.0.0.1:2001,base_srate=11.52e6" \
    log --filename $LOG_DIR/gnb.log --all_level info \
    pcap --mac_enable true --mac_filename $LOG_DIR/gnb_mac.pcap \
    > $LOG_DIR/gnb_stdout.log 2>&1 &
GNB_PID=$!

log_info "gNodeB started (PID: $GNB_PID)"

# Wait for gNodeB to initialize
sleep 3

# Check gNodeB status
if ! ps -p $GNB_PID > /dev/null; then
    log_error "gNodeB crashed during startup"
    log_info "gNodeB log tail:"
    tail -30 "$LOG_DIR/gnb_stdout.log" 2>/dev/null || echo "No log available"
    exit 1
fi

# The gNodeB will fail to connect via SCTP but should continue running
log_warn "Note: NGAP connection will fail (SCTP→TCP mismatch) but PHY layer should work"

# Step 3: Start srsUE
log_info "Step 3: Starting srsUE..."

cd /opt/srsran
export LD_LIBRARY_PATH=/opt/srsran/lib:$LD_LIBRARY_PATH

# Create temporary UE config
cp ./config/srsue/ue_nr_zmq_10mhz.conf /tmp/ue_config_mock.conf
sed -i "s|filename = /tmp/ue.log|filename = $LOG_DIR/ue.log|g" /tmp/ue_config_mock.conf
sed -i "s|mac_filename = /tmp/ue_mac.pcap|mac_filename = $LOG_DIR/ue_mac.pcap|g" /tmp/ue_config_mock.conf
sed -i "s|mac_nr_filename = /tmp/ue_mac_nr.pcap|mac_nr_filename = $LOG_DIR/ue_mac_nr.pcap|g" /tmp/ue_config_mock.conf
sed -i "s|nas_filename = /tmp/ue_nas.pcap|nas_filename = $LOG_DIR/ue_nas.pcap|g" /tmp/ue_config_mock.conf

# Start UE
/opt/srsran/bin/srsue \
    /tmp/ue_config_mock.conf \
    --rat.nr.dl_nr_arfcn 368500 \
    --rat.nr.ssb_nr_arfcn 368410 \
    --rat.nr.nof_prb 52 \
    --rat.nr.scs 15 \
    --rat.nr.ssb_scs 15 \
    > $LOG_DIR/ue_stdout.log 2>&1 &
UE_PID=$!

log_info "srsUE started (PID: $UE_PID)"

# Step 4: Monitor PHY layer activity
log_info "Step 4: Monitoring PHY layer activity (30 seconds)..."

TIMEOUT=30
PHY_SUCCESS=false

for i in $(seq 1 $TIMEOUT); do
    printf "\r[%02d/%02d] Checking PHY layer status..." "$i" "$TIMEOUT"
    
    # Check if UE found cell
    if grep -q "Found Cell.*PCI=1" "$LOG_DIR/ue.log" 2>/dev/null || \
       grep -q "Found Cell.*PCI=1" "$LOG_DIR/ue_stdout.log" 2>/dev/null; then
        echo ""
        log_info "✓ UE found cell (PCI=1) - PHY layer working!"
        PHY_SUCCESS=true
        
        # Check for additional PHY milestones
        if grep -q "SSB detected" "$LOG_DIR/ue.log" 2>/dev/null; then
            log_info "✓ SSB detected"
        fi
        
        if grep -q "PBCH decoded" "$LOG_DIR/ue.log" 2>/dev/null; then
            log_info "✓ PBCH decoded successfully"
        fi
        
        break
    fi
    
    # Check if processes are still running
    if ! ps -p $GNB_PID > /dev/null; then
        echo ""
        log_error "gNodeB crashed"
        break
    fi
    
    if ! ps -p $UE_PID > /dev/null; then
        echo ""
        log_error "UE crashed"
        break
    fi
    
    sleep 1
done

echo ""
echo "=========================================="
log_info "TEST RESULTS:"
echo "=========================================="

if [ "$PHY_SUCCESS" = "true" ]; then
    log_info "✅ PHY LAYER TEST PASSED"
    log_info "Cell detection successful - baseline established"
    
    # Show PHY layer stats
    echo ""
    log_info "PHY Layer Statistics:"
    grep -E "(SSB|PBCH|PSS|SSS|Found Cell)" "$LOG_DIR/ue.log" 2>/dev/null | tail -10 || \
    grep -E "(SSB|PBCH|PSS|SSS|Found Cell)" "$LOG_DIR/ue_stdout.log" 2>/dev/null | tail -10
    
else
    log_error "❌ PHY LAYER TEST FAILED"
    log_info "Cell detection failed"
    
    # Debug information
    echo ""
    log_info "Debug information:"
    
    echo ""
    log_info "Mock AMF log tail:"
    tail -20 "$LOG_DIR/mock_amf.log" 2>/dev/null || echo "No log available"
    
    echo ""
    log_info "gNodeB log tail:"
    tail -20 "$LOG_DIR/gnb_stdout.log" 2>/dev/null || echo "No log available"
    
    echo ""
    log_info "UE log tail:"
    tail -20 "$LOG_DIR/ue_stdout.log" 2>/dev/null || echo "No log available"
fi

echo "=========================================="
log_info "Test completed. Logs saved in: $LOG_DIR"

# Keep running for monitoring if successful
if [ "$PHY_SUCCESS" = "true" ]; then
    log_info "System running. Press Ctrl+C to stop."
    wait
fi