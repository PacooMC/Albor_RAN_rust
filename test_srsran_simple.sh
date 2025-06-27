#!/bin/bash
# Simple test for srsRAN gNodeB + srsUE with Open5GS

set +e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

log_info() { echo -e "${GREEN}[INFO]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }

# Create log directory
LOG_DIR="logs/$(date +%Y%m%d_%H%M%S)_srsran_simple"
mkdir -p "$LOG_DIR"

CONTAINER_NAME="albor-gnb-dev"

# Cleanup function
cleanup() {
    log_info "Cleaning up..."
    docker compose exec -T $CONTAINER_NAME bash -c "pkill -9 -f 'gnb' 2>/dev/null || true"
    docker compose exec -T $CONTAINER_NAME bash -c "pkill -9 -f 'srsue' 2>/dev/null || true"
    sleep 1
}

trap cleanup EXIT

log_info "=== Simple srsRAN 5G Test ==="
log_info "Log directory: $LOG_DIR"

# Initial cleanup
cleanup

# Start gNodeB with command line args (YAML parser is broken)
log_info "Starting srsRAN gNodeB..."
docker compose exec -d $CONTAINER_NAME bash -c "
    cd /workspace
    /usr/local/bin/gnb \
        --gnb_id 1 \
        cu_cp.amf.addr=127.0.0.1 \
        cu_cp.amf.port=38412 \
        cu_cp.amf.bind_addr=127.0.0.1 \
        cu_cp.amf.supported_tracking_areas.0.tac=7 \
        cu_cp.amf.supported_tracking_areas.0.plmn_list.0.plmn=00101 \
        cu_cp.amf.supported_tracking_areas.0.plmn_list.0.tai_slice_support_list.0.sst=1 \
        cell_cfg.dl_arfcn=368500 \
        cell_cfg.band=3 \
        cell_cfg.channel_bandwidth_MHz=10 \
        cell_cfg.common_scs=15 \
        cell_cfg.plmn=00101 \
        cell_cfg.tac=7 \
        cell_cfg.pci=1 \
        cell_cfg.pdcch.common.ss0_index=0 \
        cell_cfg.pdcch.common.coreset0_index=6 \
        cell_cfg.pdcch.dedicated.ss2_type=common \
        cell_cfg.pdcch.dedicated.dci_format_0_1_and_1_1=false \
        cell_cfg.prach.prach_config_index=1 \
        cell_cfg.prach.prach_root_sequence_index=1 \
        cell_cfg.prach.zero_correlation_zone=0 \
        cell_cfg.prach.prach_frequency_start=1 \
        ru_sdr.device_driver=zmq \
        ru_sdr.device_args='tx_port=tcp://127.0.0.1:2000,rx_port=tcp://127.0.0.1:2001,base_srate=11.52e6' \
        ru_sdr.srate=11.52 \
        ru_sdr.tx_gain=75 \
        ru_sdr.rx_gain=75 \
        log.filename=/workspace/$LOG_DIR/gnb.log \
        log.all_level=info \
        pcap.mac_enable=true \
        pcap.mac_filename=/workspace/$LOG_DIR/gnb_mac.pcap \
        pcap.ngap_enable=true \
        pcap.ngap_filename=/workspace/$LOG_DIR/gnb_ngap.pcap \
        > /workspace/$LOG_DIR/gnb_stdout.log 2>&1
"

# Wait for gNodeB to start
log_info "Waiting for gNodeB to connect to AMF..."
for i in {1..30}; do
    if docker compose exec -T $CONTAINER_NAME grep -q "NG setup procedure completed" "$LOG_DIR/gnb.log" 2>/dev/null || \
       docker compose exec -T $CONTAINER_NAME grep -q "Connected to AMF" "$LOG_DIR/gnb_stdout.log" 2>/dev/null; then
        log_info "✓ gNodeB connected to AMF!"
        break
    fi
    
    if [ $i -eq 30 ]; then
        log_error "gNodeB failed to connect to AMF"
        echo "gNodeB log:"
        docker compose exec -T $CONTAINER_NAME tail -20 "$LOG_DIR/gnb_stdout.log" 2>/dev/null || echo "No log"
        exit 1
    fi
    
    printf "\r[%02d/30] Waiting for AMF connection..." "$i"
    sleep 1
done
echo ""

sleep 3

# Start srsUE
log_info "Starting srsUE..."
docker compose exec -d $CONTAINER_NAME bash -c "
    export LD_LIBRARY_PATH=/usr/local/lib:\$LD_LIBRARY_PATH
    /usr/local/bin/srsue /workspace/config/srsue/ue_zmq.conf \
        --rat.nr.bands=3 \
        --rat.nr.nof_prb=52 \
        > /workspace/$LOG_DIR/ue_stdout.log 2>&1
"

# Monitor for registration
log_info "Monitoring UE registration..."
SUCCESS=false
for i in {1..60}; do
    printf "\r[%02d/60] Checking registration..." "$i"
    
    # Check for cell found
    if docker compose exec -T $CONTAINER_NAME grep -q "Found Cell" "$LOG_DIR/ue_stdout.log" 2>/dev/null; then
        if [ "$CELL_FOUND" != "true" ]; then
            echo ""
            log_info "✓ UE found cell"
            CELL_FOUND=true
        fi
    fi
    
    # Check for RRC connection
    if docker compose exec -T $CONTAINER_NAME grep -q "RRC Connected" "$LOG_DIR/ue_stdout.log" 2>/dev/null; then
        if [ "$RRC_CONNECTED" != "true" ]; then
            echo ""
            log_info "✓ RRC connected"
            RRC_CONNECTED=true
        fi
    fi
    
    # Check for registration
    if docker compose exec -T $CONTAINER_NAME grep -q "NAS-5G.*Registration complete" "$LOG_DIR/ue_stdout.log" 2>/dev/null || \
       docker compose exec -T $CONTAINER_NAME grep -q "5GMM-REGISTERED" "/var/log/open5gs/amf.log" 2>/dev/null; then
        SUCCESS=true
        echo ""
        log_info "✓ UE registered to 5G network!"
        break
    fi
    
    sleep 1
done

echo ""
echo "=========================================="
log_info "TEST RESULTS:"
echo "=========================================="

if [ "$SUCCESS" = "true" ]; then
    log_info "✅ SUCCESS: UE registered to 5G network!"
else
    log_error "❌ FAILED: UE did not register"
    
    echo ""
    log_info "gNodeB log tail:"
    docker compose exec -T $CONTAINER_NAME tail -20 "$LOG_DIR/gnb_stdout.log" 2>/dev/null || echo "No log"
    
    echo ""
    log_info "UE log tail:"
    docker compose exec -T $CONTAINER_NAME tail -20 "$LOG_DIR/ue_stdout.log" 2>/dev/null || echo "No log"
fi

echo "=========================================="