#!/bin/bash
# test_srsran_compliant.sh - CLAUDE.md compliant test for srsRAN gNodeB + UE
# NO CORE NETWORK - Uses --no_core option
# This replaces the non-compliant test_srsran.sh

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
LOG_DIR="/workspace/logs/$(date +%Y%m%d_%H%M%S)_srsran_compliant"
mkdir -p "$LOG_DIR"

log_info "=== srsRAN Compliant Test (NO CORE NETWORK) ==="
log_info "Testing: srsRAN gNodeB + srsRAN UE in standalone mode"
log_info "Log directory: $LOG_DIR"

# Check if we're running inside the container
if [ -f /.dockerenv ]; then
    IN_DOCKER=1
    log_info "Running inside Docker container"
else
    log_info "Running test through Docker exec"
fi

# Cleanup function
cleanup() {
    log_info "Performing cleanup..."
    
    # Kill gnb and srsue processes
    if [ "$IN_DOCKER" = "1" ]; then
        pkill -9 -f gnb 2>/dev/null || true
        pkill -9 -f srsue 2>/dev/null || true
    else
        docker exec albor-gnb-dev bash -c "pkill -9 -f gnb 2>/dev/null || true"
        docker exec albor-gnb-dev bash -c "pkill -9 -f srsue 2>/dev/null || true"
    fi
    
    sleep 1
}

# Set trap for cleanup on exit
trap cleanup EXIT

# Initial cleanup
log_info "Initial cleanup of existing processes..."
cleanup

# Step 1: Start gNodeB in standalone mode
log_info "Step 1: Starting srsRAN gNodeB in standalone mode (--no_core)..."

# The srsRAN binary doesn't support YAML parsing properly, use command line
if [ "$IN_DOCKER" = "1" ]; then
    cd /opt/srsran_project
    /opt/srsran_project/bin/gnb \
        --gnb_id 1 \
        cu_cp amf --no_core true \
        cell_cfg --dl_arfcn 368500 --band 3 --channel_bandwidth_MHz 10 \
        cell_cfg --common_scs 15kHz --plmn 00101 --tac 7 --pci 1 \
        ru_sdr --device_driver zmq --srate 11.52 --tx_gain 75 --rx_gain 75 \
        ru_sdr --device_args "tx_port=tcp://127.0.0.1:2000,rx_port=tcp://127.0.0.1:2001,base_srate=11.52e6" \
        log --filename $LOG_DIR/gnb.log --all_level info \
        pcap --mac_enable true --mac_filename $LOG_DIR/gnb_mac.pcap \
        > $LOG_DIR/gnb_stdout.log 2>&1 &
    GNB_PID=$!
else
    docker exec albor-gnb-dev bash -c "
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
            > $LOG_DIR/gnb_stdout.log 2>&1 &
        echo \$!
    " > /tmp/gnb_pid.txt
    GNB_PID=$(cat /tmp/gnb_pid.txt)
fi

log_info "gNodeB started (PID: $GNB_PID)"

# Wait for gNodeB to initialize
log_info "Waiting for gNodeB to initialize..."
sleep 5

# Check if gNodeB is running
GNB_RUNNING=false
for i in {1..10}; do
    if [ "$IN_DOCKER" = "1" ]; then
        if kill -0 $GNB_PID 2>/dev/null; then
            GNB_RUNNING=true
            break
        fi
    else
        if docker exec albor-gnb-dev bash -c "kill -0 $GNB_PID 2>/dev/null"; then
            GNB_RUNNING=true
            break
        fi
    fi
    printf "\r[%02d/10] Waiting for gNodeB..." "$i"
    sleep 1
done
echo ""

if [ "$GNB_RUNNING" = "false" ]; then
    log_error "✗ gNodeB failed to start"
    log_info "gNodeB log tail:"
    if [ "$IN_DOCKER" = "1" ]; then
        tail -30 "$LOG_DIR/gnb_stdout.log" 2>/dev/null || echo "No logs available"
    else
        docker exec albor-gnb-dev bash -c "tail -30 '$LOG_DIR/gnb_stdout.log' 2>/dev/null || echo 'No logs available'"
    fi
    exit 1
fi

log_info "✓ gNodeB is running in standalone mode!"

# Give gNodeB time to stabilize
sleep 3

# Step 2: Start srsUE
log_info "Step 2: Starting srsUE with NR configuration..."

if [ "$IN_DOCKER" = "1" ]; then
    cd /opt/srsran
    export LD_LIBRARY_PATH=/opt/srsran/lib:$LD_LIBRARY_PATH
    
    # Create a temporary UE config with updated log paths
    cp /workspace/config/srsue/ue_nr_zmq_10mhz.conf /tmp/ue_config.conf
    sed -i "s|filename = /tmp/ue.log|filename = $LOG_DIR/ue.log|g" /tmp/ue_config.conf
    sed -i "s|mac_filename = /tmp/ue_mac.pcap|mac_filename = $LOG_DIR/ue_mac.pcap|g" /tmp/ue_config.conf
    sed -i "s|mac_nr_filename = /tmp/ue_mac_nr.pcap|mac_nr_filename = $LOG_DIR/ue_mac_nr.pcap|g" /tmp/ue_config.conf
    sed -i "s|nas_filename = /tmp/ue_nas.pcap|nas_filename = $LOG_DIR/ue_nas.pcap|g" /tmp/ue_config.conf
    
    # Use the definitive 10 MHz configuration
    /opt/srsran/bin/srsue \
        /tmp/ue_config.conf \
        --rat.nr.dl_nr_arfcn 368500 \
        --rat.nr.ssb_nr_arfcn 368410 \
        --rat.nr.nof_prb 52 \
        --rat.nr.scs 15 \
        --rat.nr.ssb_scs 15 \
        > $LOG_DIR/ue_stdout.log 2>&1 &
    UE_PID=$!
else
    docker exec albor-gnb-dev bash -c "
        cd /opt/srsran
        export LD_LIBRARY_PATH=/opt/srsran/lib:\$LD_LIBRARY_PATH
        
        # Create a temporary UE config with updated log paths
        cp /workspace/config/srsue/ue_nr_zmq_10mhz.conf /tmp/ue_config.conf
        sed -i \"s|filename = /tmp/ue.log|filename = $LOG_DIR/ue.log|g\" /tmp/ue_config.conf
        sed -i \"s|mac_filename = /tmp/ue_mac.pcap|mac_filename = $LOG_DIR/ue_mac.pcap|g\" /tmp/ue_config.conf
        sed -i \"s|mac_nr_filename = /tmp/ue_mac_nr.pcap|mac_nr_filename = $LOG_DIR/ue_mac_nr.pcap|g\" /tmp/ue_config.conf
        sed -i \"s|nas_filename = /tmp/ue_nas.pcap|nas_filename = $LOG_DIR/ue_nas.pcap|g\" /tmp/ue_config.conf
        
        /opt/srsran/bin/srsue \
            /tmp/ue_config.conf \
            --rat.nr.dl_nr_arfcn 368500 \
            --rat.nr.ssb_nr_arfcn 368410 \
            --rat.nr.nof_prb 52 \
            --rat.nr.scs 15 \
            --rat.nr.ssb_scs 15 \
            > $LOG_DIR/ue_stdout.log 2>&1 &
        echo \$!
    " > /tmp/ue_pid.txt
    UE_PID=$(cat /tmp/ue_pid.txt)
fi

log_info "srsUE started (PID: $UE_PID)"

# Step 3: Monitor connection
log_info "Step 3: Monitoring for cell detection and RRC connection..."

TIMEOUT=60
SUCCESS=false
MILESTONES=()

# Helper function to check logs
check_log() {
    local pattern="$1"
    local file="$2"
    if [ "$IN_DOCKER" = "1" ]; then
        grep -q "$pattern" "$file" 2>/dev/null
    else
        docker exec albor-gnb-dev grep -q "$pattern" "$file" 2>/dev/null
    fi
}

# Monitor connection progress
for i in $(seq 1 $TIMEOUT); do
    printf "\r[%02d/%02d] Checking connection status..." "$i" "$TIMEOUT"
    
    # Check UE logs
    UE_LOG="$LOG_DIR/ue.log"
    
    # Check various stages
    if check_log "Found Cell.*PCI=1" "$UE_LOG" || check_log "Found Cell.*PCI=1" "$LOG_DIR/ue_stdout.log"; then
        if [[ ! " ${MILESTONES[@]} " =~ " cell_found " ]]; then
            echo ""
            log_info "✓ UE found cell (PCI=1)"
            MILESTONES+=("cell_found")
        fi
    fi
    
    if check_log "Random Access Complete" "$UE_LOG" || check_log "Random Access Complete" "$LOG_DIR/ue_stdout.log"; then
        if [[ ! " ${MILESTONES[@]} " =~ " rach_complete " ]]; then
            echo ""
            log_info "✓ Random access completed"
            MILESTONES+=("rach_complete")
        fi
    fi
    
    if check_log "RRC Connected" "$UE_LOG" || check_log "RRC Connected" "$LOG_DIR/ue_stdout.log"; then
        if [[ ! " ${MILESTONES[@]} " =~ " rrc_connected " ]]; then
            echo ""
            log_info "✓ RRC connected"
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

# Helper function to show logs
show_log() {
    local file="$1"
    local lines="${2:-20}"
    if [ "$IN_DOCKER" = "1" ]; then
        if [ -f "$file" ]; then
            tail -$lines "$file" 2>/dev/null || echo "  (log file not found)"
        else
            echo "  (log file not found: $file)"
        fi
    else
        docker exec albor-gnb-dev bash -c "
            if [ -f '$file' ]; then
                tail -$lines '$file' 2>/dev/null || echo '  (log file not found)'
            else
                echo '  (log file not found: $file)'
            fi
        "
    fi
}

if [ "$SUCCESS" = "true" ]; then
    log_info "✅ SUCCESS: Direct gNodeB ↔ UE connection established!"
    log_info "✅ NO CORE NETWORK REQUIRED!"
    
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
    if [ ${#MILESTONES[@]} -gt 0 ]; then
        log_warn "⚠️  Partial success - some milestones achieved"
    else
        log_error "❌ FAILED: No connection established"
    fi
    
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
    show_log "$LOG_DIR/gnb.log" 30
    
    echo ""
    log_info "UE log tail:"
    show_log "$LOG_DIR/ue.log" 30
    
    echo ""
    log_info "Process status:"
    if [ "$IN_DOCKER" = "1" ]; then
        ps aux | grep -E "(gnb|srsue)" | grep -v grep || echo "  No relevant processes found"
    else
        docker exec albor-gnb-dev bash -c "ps aux | grep -E '(gnb|srsue)' | grep -v grep" || echo "  No relevant processes found"
    fi
fi

echo "=========================================="
log_info "Logs saved to: $LOG_DIR"

# Keep running if successful for monitoring
if [ "$SUCCESS" = "true" ]; then
    log_info "System is running. Press Ctrl+C to stop."
    log_info "Monitor logs in: $LOG_DIR"
    
    # Wait for user interrupt
    wait
fi