#!/bin/bash
# test_srsran_nocore.sh - Test srsRAN without core network
# This bypasses AMF requirements for PHY layer testing

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

# Create log directory on host
LOG_DIR="./logs/$(date +%Y%m%d_%H%M%S)_srsran_nocore"
mkdir -p "$LOG_DIR"

log_info "=== srsRAN No-Core Test (PHY Layer Only) ==="
log_info "Log directory: $LOG_DIR"

# Cleanup function
cleanup() {
    log_info "Cleaning up processes..."
    docker exec albor-gnb-dev bash -c "pkill -9 -f gnb 2>/dev/null || true"
    docker exec albor-gnb-dev bash -c "pkill -9 -f srsue 2>/dev/null || true"
    docker exec albor-gnb-dev bash -c "pkill -9 -f mock_amf 2>/dev/null || true"
    sleep 1
}

# Set trap
trap cleanup EXIT

# Initial cleanup
cleanup

# Step 1: Start Mock AMF inside container
log_info "Step 1: Starting Mock AMF (TCP fallback)..."

docker exec -d albor-gnb-dev python3 /workspace/scripts/mock_amf.py 127.0.0.4 38412
sleep 2

# Verify mock AMF is running
if docker exec albor-gnb-dev netstat -tuln 2>/dev/null | grep -q "127.0.0.4:38412"; then
    log_info "✓ Mock AMF listening on 127.0.0.4:38412 (TCP)"
else
    log_error "Failed to start Mock AMF"
    exit 1
fi

# Step 2: Start srsRAN gNodeB inside container
log_info "Step 2: Starting srsRAN gNodeB..."

# Create a script to run inside container
cat > /tmp/run_gnb.sh << 'EOF'
#!/bin/bash
cd /opt/srsran_project
/opt/srsran_project/bin/gnb \
    --gnb_id 1 \
    cu_cp amf --addr 127.0.0.4 --port 38412 --bind_addr 127.0.0.11 \
    cell_cfg --dl_arfcn 368500 --band 3 --channel_bandwidth_MHz 10 \
    cell_cfg --common_scs 15kHz --plmn 00101 --tac 7 --pci 1 \
    ru_sdr --device_driver zmq --srate 11.52 --tx_gain 75 --rx_gain 75 \
    ru_sdr --device_args "tx_port=tcp://127.0.0.1:2000,rx_port=tcp://127.0.0.1:2001,base_srate=11.52e6" \
    log --filename /workspace/logs/gnb.log --all_level info \
    pcap --mac_enable true --mac_filename /workspace/logs/gnb_mac.pcap \
    > /workspace/logs/gnb_stdout.log 2>&1 &
echo $!
EOF

# Copy and execute
docker cp /tmp/run_gnb.sh albor-gnb-dev:/tmp/
docker exec albor-gnb-dev chmod +x /tmp/run_gnb.sh
GNB_PID=$(docker exec albor-gnb-dev /tmp/run_gnb.sh)

log_info "gNodeB started (PID: $GNB_PID)"
sleep 3

# Check if gNodeB is running
if ! docker exec albor-gnb-dev ps -p $GNB_PID > /dev/null 2>&1; then
    log_error "gNodeB failed to start"
    docker exec albor-gnb-dev cat /workspace/logs/gnb_stdout.log 2>/dev/null | tail -30
    exit 1
fi

log_warn "Note: NGAP connection will fail (SCTP→TCP) but PHY should work"

# Step 3: Start srsUE inside container
log_info "Step 3: Starting srsUE..."

cat > /tmp/run_ue.sh << 'EOF'
#!/bin/bash
cd /opt/srsran
export LD_LIBRARY_PATH=/opt/srsran/lib:$LD_LIBRARY_PATH

# Copy and modify config
cp /workspace/config/srsue/ue_nr_zmq_10mhz.conf /tmp/ue_config.conf
sed -i "s|filename = /tmp/ue.log|filename = /workspace/logs/ue.log|g" /tmp/ue_config.conf
sed -i "s|mac_filename = /tmp/ue_mac.pcap|mac_filename = /workspace/logs/ue_mac.pcap|g" /tmp/ue_config.conf

/opt/srsran/bin/srsue \
    /tmp/ue_config.conf \
    --rat.nr.dl_nr_arfcn 368500 \
    --rat.nr.ssb_nr_arfcn 368410 \
    --rat.nr.nof_prb 52 \
    --rat.nr.scs 15 \
    --rat.nr.ssb_scs 15 \
    > /workspace/logs/ue_stdout.log 2>&1 &
echo $!
EOF

docker cp /tmp/run_ue.sh albor-gnb-dev:/tmp/
docker exec albor-gnb-dev chmod +x /tmp/run_ue.sh
UE_PID=$(docker exec albor-gnb-dev /tmp/run_ue.sh)

log_info "srsUE started (PID: $UE_PID)"

# Step 4: Monitor PHY layer
log_info "Step 4: Monitoring PHY layer (30 seconds)..."

TIMEOUT=30
PHY_SUCCESS=false

for i in $(seq 1 $TIMEOUT); do
    printf "\r[%02d/%02d] Checking PHY status..." "$i" "$TIMEOUT"
    
    # Check for cell detection in container logs
    if docker exec albor-gnb-dev grep -q "Found Cell.*PCI=1" /workspace/logs/ue.log 2>/dev/null || \
       docker exec albor-gnb-dev grep -q "Found Cell.*PCI=1" /workspace/logs/ue_stdout.log 2>/dev/null; then
        echo ""
        log_info "✓ UE found cell (PCI=1) - PHY layer working!"
        PHY_SUCCESS=true
        break
    fi
    
    # Check process status
    if ! docker exec albor-gnb-dev ps -p $GNB_PID > /dev/null 2>&1; then
        echo ""
        log_error "gNodeB crashed"
        break
    fi
    
    if ! docker exec albor-gnb-dev ps -p $UE_PID > /dev/null 2>&1; then
        echo ""
        log_error "UE crashed"
        break
    fi
    
    sleep 1
done

echo ""

# Copy logs from container to host
log_info "Copying logs from container..."
docker cp albor-gnb-dev:/workspace/logs/. "$LOG_DIR/" 2>/dev/null || true

echo "=========================================="
log_info "TEST RESULTS:"
echo "=========================================="

if [ "$PHY_SUCCESS" = "true" ]; then
    log_info "✅ PHY LAYER TEST PASSED"
    log_info "srsRAN baseline established - cell detection working"
    
    # Show relevant logs
    echo ""
    log_info "Cell detection logs:"
    grep -E "(Found Cell|SSB|PBCH)" "$LOG_DIR/ue.log" 2>/dev/null | tail -10 || \
    grep -E "(Found Cell|SSB|PBCH)" "$LOG_DIR/ue_stdout.log" 2>/dev/null | tail -10
else
    log_error "❌ PHY LAYER TEST FAILED"
    log_info "No cell detection"
    
    # Debug info
    echo ""
    log_info "Debug information:"
    echo ""
    log_info "gNodeB log tail:"
    tail -20 "$LOG_DIR/gnb_stdout.log" 2>/dev/null || echo "No log available"
    echo ""
    log_info "UE log tail:"
    tail -20 "$LOG_DIR/ue_stdout.log" 2>/dev/null || echo "No log available"
fi

echo "=========================================="
log_info "Logs saved in: $LOG_DIR"

# Keep running if successful
if [ "$PHY_SUCCESS" = "true" ]; then
    log_info "System running. Press Ctrl+C to stop."
    
    # Monitor logs
    while true; do
        sleep 5
        # Copy latest logs
        docker cp albor-gnb-dev:/workspace/logs/. "$LOG_DIR/" 2>/dev/null || true
    done
fi