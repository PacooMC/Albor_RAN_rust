#!/bin/bash
# test_srsran_standalone.sh - Test srsRAN gNodeB in standalone mode WITHOUT Open5GS
# This script demonstrates srsRAN running with --no_core option

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
LOG_DIR="/home/fmc/Albor_RAN_rust/logs/$(date +%Y%m%d_%H%M%S)_srsran_standalone"
mkdir -p "$LOG_DIR"

log_info "=== srsRAN Standalone Test (NO CORE NETWORK) ==="
log_info "Testing: srsRAN gNodeB + srsRAN UE without Open5GS"
log_info "Log directory: $LOG_DIR"

# Cleanup function
cleanup() {
    log_info "Performing cleanup..."
    docker exec albor-gnb-dev bash -c "pkill -9 -f gnb 2>/dev/null || true"
    docker exec albor-gnb-dev bash -c "pkill -9 -f srsue 2>/dev/null || true"
    sleep 1
}

trap cleanup EXIT

# Initial cleanup
log_info "Initial cleanup of existing processes..."
cleanup

# Step 1: Start gNodeB with --no_core option
log_info "Step 1: Starting srsRAN gNodeB in standalone mode (--no_core)..."

# Start gNodeB with command line parameters and --no_core
docker exec -d albor-gnb-dev bash -c "
    cd /opt/srsran_project
    /opt/srsran_project/bin/gnb \
        --gnb_id 1 \
        cu_cp amf --no_core true \
        cell_cfg --dl_arfcn 368500 --band 3 --channel_bandwidth_MHz 10 \
        cell_cfg --common_scs 15kHz --plmn 00101 --tac 7 --pci 1 \
        ru_sdr --device_driver zmq --srate 11.52 --tx_gain 75 --rx_gain 75 \
        ru_sdr --device_args 'tx_port=tcp://127.0.0.1:2000,rx_port=tcp://127.0.0.1:2001,base_srate=11.52e6' \
        log --filename $LOG_DIR/gnb.log --all_level info \
        pcap --mac_enable true --mac_filename $LOG_DIR/gnb_mac.pcap \
        > $LOG_DIR/gnb_stdout.log 2>&1
"

# Wait for gNodeB to start
log_info "Waiting for gNodeB to initialize..."
sleep 5

# Check if gNodeB is running
if docker exec albor-gnb-dev bash -c "pgrep -f gnb > /dev/null"; then
    log_info "✓ gNodeB is running in standalone mode!"
else
    log_error "✗ gNodeB failed to start"
    if [ -f "$LOG_DIR/gnb_stdout.log" ]; then
        log_error "gNodeB output:"
        docker exec albor-gnb-dev cat "$LOG_DIR/gnb_stdout.log" | tail -20
    fi
    exit 1
fi

# Step 2: Start srsUE
log_info "Step 2: Starting srsUE..."

# Create a temporary UE config with updated log paths
docker exec albor-gnb-dev bash -c "
    cp /workspace/config/srsue/ue_nr_zmq_10mhz.conf /tmp/ue_config.conf
    sed -i 's|filename = /tmp/ue.log|filename = $LOG_DIR/ue.log|g' /tmp/ue_config.conf
    sed -i 's|mac_filename = /tmp/ue_mac.pcap|mac_filename = $LOG_DIR/ue_mac.pcap|g' /tmp/ue_config.conf
    sed -i 's|mac_nr_filename = /tmp/ue_mac_nr.pcap|mac_nr_filename = $LOG_DIR/ue_mac_nr.pcap|g' /tmp/ue_config.conf
    sed -i 's|nas_filename = /tmp/ue_nas.pcap|nas_filename = $LOG_DIR/ue_nas.pcap|g' /tmp/ue_config.conf
"

# Start UE
docker exec -d albor-gnb-dev bash -c "
    cd /opt/srsran
    export LD_LIBRARY_PATH=/opt/srsran/lib:\$LD_LIBRARY_PATH
    
    /opt/srsran/bin/srsue \
        /tmp/ue_config.conf \
        --rat.nr.dl_nr_arfcn 368500 \
        --rat.nr.ssb_nr_arfcn 368410 \
        --rat.nr.nof_prb 52 \
        --rat.nr.scs 15 \
        --rat.nr.ssb_scs 15 \
        > $LOG_DIR/ue_stdout.log 2>&1
"

log_info "srsUE started"

# Step 3: Monitor for connection
log_info "Step 3: Monitoring for cell detection and RRC connection..."

TIMEOUT=30
SUCCESS=false
MILESTONES=()

for i in $(seq 1 $TIMEOUT); do
    printf "\r[%02d/%02d] Checking connection status..." "$i" "$TIMEOUT"
    
    # Check UE log for milestones
    if docker exec albor-gnb-dev bash -c "grep -q 'Found Cell.*PCI=1' '$LOG_DIR/ue.log' 2>/dev/null || grep -q 'Found Cell.*PCI=1' '$LOG_DIR/ue_stdout.log' 2>/dev/null"; then
        if [[ ! " ${MILESTONES[@]} " =~ " cell_found " ]]; then
            echo ""
            log_info "✓ UE found cell (PCI=1)"
            MILESTONES+=("cell_found")
        fi
    fi
    
    if docker exec albor-gnb-dev bash -c "grep -q 'Random Access Complete' '$LOG_DIR/ue.log' 2>/dev/null || grep -q 'Random Access Complete' '$LOG_DIR/ue_stdout.log' 2>/dev/null"; then
        if [[ ! " ${MILESTONES[@]} " =~ " rach_complete " ]]; then
            echo ""
            log_info "✓ Random access completed"
            MILESTONES+=("rach_complete")
        fi
    fi
    
    if docker exec albor-gnb-dev bash -c "grep -q 'RRC Connected' '$LOG_DIR/ue.log' 2>/dev/null || grep -q 'RRC Connected' '$LOG_DIR/ue_stdout.log' 2>/dev/null"; then
        if [[ ! " ${MILESTONES[@]} " =~ " rrc_connected " ]]; then
            echo ""
            log_info "✓ RRC connected!"
            MILESTONES+=("rrc_connected")
            SUCCESS=true
            break
        fi
    fi
    
    sleep 1
done

echo ""
echo "=========================================="
log_info "TEST RESULTS:"
echo "=========================================="

if [ "$SUCCESS" = "true" ]; then
    log_info "✅ SUCCESS: RRC connection established WITHOUT core network!"
    
    # Show milestones
    echo ""
    log_info "Connection milestones achieved:"
    for milestone in "${MILESTONES[@]}"; do
        case $milestone in
            cell_found) echo "  ✓ Cell detection" ;;
            rach_complete) echo "  ✓ Random access procedure" ;;
            rrc_connected) echo "  ✓ RRC connection establishment" ;;
        esac
    done
else
    log_warn "⚠️  Partial success or timeout"
    
    # Show milestones achieved
    echo ""
    log_info "Milestones achieved:"
    if [ ${#MILESTONES[@]} -eq 0 ]; then
        echo "  ✗ No milestones achieved"
    else
        for milestone in "${MILESTONES[@]}"; do
            case $milestone in
                cell_found) echo "  ✓ Cell detection" ;;
                rach_complete) echo "  ✓ Random access procedure" ;;
                rrc_connected) echo "  ✓ RRC connection establishment" ;;
            esac
        done
    fi
    
    # Debug information
    echo ""
    log_info "Debug information:"
    
    echo ""
    log_info "gNodeB log tail:"
    docker exec albor-gnb-dev bash -c "tail -20 '$LOG_DIR/gnb.log' 2>/dev/null || tail -20 '$LOG_DIR/gnb_stdout.log' 2>/dev/null || echo 'No logs available'"
    
    echo ""
    log_info "UE log tail:"
    docker exec albor-gnb-dev bash -c "tail -20 '$LOG_DIR/ue.log' 2>/dev/null || tail -20 '$LOG_DIR/ue_stdout.log' 2>/dev/null || echo 'No logs available'"
fi

echo "=========================================="
log_info "Full logs saved to: $LOG_DIR"

# Keep running for a bit to see if connection is maintained
if [ "$SUCCESS" = "true" ]; then
    log_info "Monitoring connection for 10 more seconds..."
    sleep 10
fi