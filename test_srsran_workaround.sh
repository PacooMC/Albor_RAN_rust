#!/bin/bash
# test_srsran_workaround.sh - Test srsRAN with Open5GS workaround
# Uses the workaround setup for Open5GS due to Docker permission limitations

set +e

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
LOG_DIR="/workspace/logs/$(date +%Y%m%d_%H%M%S)_srsran_workaround"
mkdir -p "$LOG_DIR"

log_info "=== srsRAN Test with Open5GS Workaround ==="
log_info "Testing: AMF (workaround) + srsRAN gNodeB + srsRAN UE"
log_info "Log directory: $LOG_DIR"

# Cleanup function
cleanup() {
    log_info "Performing cleanup..."
    
    # Kill gnb and srsue processes
    docker exec albor-gnb-dev bash -c "pkill -9 -f 'gnb' 2>/dev/null || true"
    docker exec albor-gnb-dev bash -c "pkill -9 -f 'srsue' 2>/dev/null || true"
    
    # Kill processes on ZMQ ports
    for port in 2000 2001; do
        docker exec albor-gnb-dev bash -c "
            PID=\$(lsof -ti:$port 2>/dev/null || true)
            if [ ! -z \"\$PID\" ]; then
                kill -9 \$PID 2>/dev/null || true
            fi
        " 2>/dev/null || true
    done
    
    sleep 2
}

# Set trap for cleanup on exit
trap cleanup EXIT

# Initial cleanup
log_info "Initial cleanup of existing processes..."
cleanup

# Step 1: Check if AMF is running from workaround
log_info "Step 1: Checking AMF from workaround setup..."

AMF_RUNNING=false
if docker exec albor-gnb-dev bash -c "netstat -tuln 2>/dev/null | grep -q '127.0.0.5:38412'"; then
    log_info "✓ AMF is already running on 127.0.0.5:38412"
    AMF_RUNNING=true
else
    log_warn "AMF not running - running workaround setup..."
    docker exec albor-gnb-dev /workspace/scripts/open5gs_workaround.sh
    
    # Check again
    sleep 3
    if docker exec albor-gnb-dev bash -c "netstat -tuln 2>/dev/null | grep -q '127.0.0.5:38412'"; then
        log_info "✓ AMF started successfully"
        AMF_RUNNING=true
    else
        log_error "Failed to start AMF"
        exit 1
    fi
fi

# Find the latest Open5GS log directory
OPEN5GS_LOG_DIR=$(docker exec albor-gnb-dev bash -c "ls -td /workspace/logs/open5gs_* 2>/dev/null | head -1")
if [ ! -z "$OPEN5GS_LOG_DIR" ]; then
    log_info "Using Open5GS logs from: $OPEN5GS_LOG_DIR"
fi

# Step 2: Start srsRAN gNodeB
log_info "Step 2: Starting srsRAN gNodeB..."

# Start gNodeB with command line parameters (YAML parser is broken)
docker exec albor-gnb-dev bash -c "
    cd /opt/srsran_project
    /opt/srsran_project/bin/gnb \
        --gnb_id 1 \
        cu_cp amf --addr 127.0.0.5 --port 38412 --bind_addr 127.0.0.11 \
        cell_cfg --dl_arfcn 368500 --band 3 --channel_bandwidth_MHz 10 \
        cell_cfg --common_scs 15kHz --plmn 00101 --tac 7 --pci 1 \
        ru_sdr --device_driver zmq --srate 11.52 --tx_gain 75 --rx_gain 75 \
        ru_sdr --device_args 'tx_port=tcp://127.0.0.1:2000,rx_port=tcp://127.0.0.1:2001,base_srate=11.52e6' \
        log --filename $LOG_DIR/gnb.log --all_level info \
        pcap --mac_enable true --mac_filename $LOG_DIR/gnb_mac.pcap \
        pcap --ngap_enable true --ngap_filename $LOG_DIR/gnb_ngap.pcap \
        > $LOG_DIR/gnb_stdout.log 2>&1 &
    echo \$!
" > /tmp/gnb_pid.txt
GNB_PID=$(cat /tmp/gnb_pid.txt)

log_info "gNodeB started (PID: $GNB_PID)"

# Wait for AMF connection
log_info "Waiting for gNodeB to connect to AMF..."
CONNECTED=false
for i in {1..30}; do
    if docker exec albor-gnb-dev bash -c "grep -q 'NG setup procedure completed' '$LOG_DIR/gnb.log' 2>/dev/null || grep -q 'NG setup procedure completed' '$LOG_DIR/gnb_stdout.log' 2>/dev/null"; then
        log_info "✓ gNodeB connected to AMF successfully!"
        CONNECTED=true
        break
    fi
    
    # Also check AMF logs
    if [ ! -z "$OPEN5GS_LOG_DIR" ]; then
        if docker exec albor-gnb-dev bash -c "grep -q 'NG setup request' '$OPEN5GS_LOG_DIR/amf.log' 2>/dev/null"; then
            log_info "✓ AMF received NG setup request"
            if docker exec albor-gnb-dev bash -c "grep -q 'NG setup response' '$OPEN5GS_LOG_DIR/amf.log' 2>/dev/null"; then
                log_info "✓ AMF sent NG setup response"
                CONNECTED=true
                break
            fi
        fi
    fi
    
    printf "\r[%02d/30] Waiting for NG setup completion..." "$i"
    sleep 1
done
echo ""

if [ "$CONNECTED" = "false" ]; then
    log_error "✗ gNodeB failed to connect to AMF"
    log_info "gNodeB log tail:"
    docker exec albor-gnb-dev bash -c "tail -30 '$LOG_DIR/gnb.log' 2>/dev/null || tail -30 '$LOG_DIR/gnb_stdout.log'"
    
    log_info "AMF log tail:"
    if [ ! -z "$OPEN5GS_LOG_DIR" ]; then
        docker exec albor-gnb-dev bash -c "tail -30 '$OPEN5GS_LOG_DIR/amf.log' 2>/dev/null"
    fi
    exit 1
fi

# Give gNodeB time to stabilize
sleep 3

# Step 3: Start srsUE
log_info "Step 3: Starting srsUE with NR configuration..."

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

log_info "srsUE started (PID: $UE_PID)"

# Step 4: Monitor connection
log_info "Step 4: Monitoring connection..."

TIMEOUT=60
SUCCESS=false
MILESTONES=()

for i in $(seq 1 $TIMEOUT); do
    printf "\r[%02d/%02d] Checking connection status..." "$i" "$TIMEOUT"
    
    UE_LOG="$LOG_DIR/ue.log"
    
    # Check various stages
    if docker exec albor-gnb-dev bash -c "grep -q 'Found Cell.*PCI=1' '$UE_LOG' 2>/dev/null || grep -q 'Found Cell.*PCI=1' '$LOG_DIR/ue_stdout.log' 2>/dev/null"; then
        if [[ ! " ${MILESTONES[@]} " =~ " cell_found " ]]; then
            echo ""
            log_info "✓ UE found cell (PCI=1)"
            MILESTONES+=("cell_found")
        fi
    fi
    
    if docker exec albor-gnb-dev bash -c "grep -q 'Random Access Complete' '$UE_LOG' 2>/dev/null || grep -q 'Random Access Complete' '$LOG_DIR/ue_stdout.log' 2>/dev/null"; then
        if [[ ! " ${MILESTONES[@]} " =~ " rach_complete " ]]; then
            echo ""
            log_info "✓ Random access completed"
            MILESTONES+=("rach_complete")
        fi
    fi
    
    if docker exec albor-gnb-dev bash -c "grep -q 'RRC Connected' '$UE_LOG' 2>/dev/null || grep -q 'RRC Connected' '$LOG_DIR/ue_stdout.log' 2>/dev/null"; then
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

if [ "$SUCCESS" = "true" ]; then
    log_info "✅ SUCCESS: RRC connection established!"
    
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
    
    # Note about NAS registration
    echo ""
    log_warn "Note: NAS registration may not complete due to missing Open5GS components"
    log_info "However, RRC connection proves the baseline is working!"
    
else
    log_error "❌ FAILED: No RRC connection within ${TIMEOUT} seconds"
    
    # Debug information
    echo ""
    log_info "Debug information:"
    
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
    
    echo ""
    log_info "gNodeB log tail:"
    docker exec albor-gnb-dev bash -c "tail -30 '$LOG_DIR/gnb.log' 2>/dev/null || tail -30 '$LOG_DIR/gnb_stdout.log' 2>/dev/null"
    
    echo ""
    log_info "UE log tail:"
    docker exec albor-gnb-dev bash -c "tail -30 '$LOG_DIR/ue.log' 2>/dev/null || tail -30 '$LOG_DIR/ue_stdout.log' 2>/dev/null"
fi

echo "=========================================="

# Keep running if successful for monitoring
if [ "$SUCCESS" = "true" ]; then
    log_info "System is running. Press Ctrl+C to stop."
    log_info "Monitor logs in: $LOG_DIR"
    log_info "Open5GS logs in: $OPEN5GS_LOG_DIR"
    
    # Save success marker
    echo "RRC_CONNECTED" > "$LOG_DIR/success.marker"
    
    # Wait for user interrupt
    wait
fi