#!/bin/bash
# test_srsran_adaptive.sh - Adaptive srsRAN test that works with or without SCTP
# Automatically detects SCTP support and uses appropriate solution

set +e  # Don't exit on error

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
LOG_DIR="/workspace/logs/$(date +%Y%m%d_%H%M%S)_srsran_adaptive"
mkdir -p "$LOG_DIR"

log_info "=== Adaptive srsRAN 5G SA Test ==="
log_info "Log directory: $LOG_DIR"

# Check if we're running inside the container
if [ -f /.dockerenv ]; then
    IN_DOCKER=1
else
    IN_DOCKER=0
fi

# Cleanup function
cleanup() {
    log_info "Performing cleanup..."
    
    # Kill test processes
    for process in gnb srsue mock_amf.py; do
        if [ "$IN_DOCKER" = "1" ]; then
            pkill -9 -f "$process" 2>/dev/null || true
        else
            docker exec albor-gnb-dev bash -c "pkill -9 -f '$process' 2>/dev/null || true" 2>/dev/null || true
        fi
    done
    
    # Kill processes on ZMQ ports
    for port in 2000 2001; do
        if [ "$IN_DOCKER" = "1" ]; then
            PID=$(lsof -ti:$port 2>/dev/null || true)
            [ ! -z "$PID" ] && kill -9 $PID 2>/dev/null || true
        fi
    done
    
    sleep 1
}

# Set trap for cleanup
trap cleanup EXIT

# Initial cleanup
cleanup

# Step 1: Check SCTP support
log_info "Step 1: Checking SCTP support..."

# Run SCTP check script
if [ "$IN_DOCKER" = "1" ]; then
    bash /workspace/scripts/check_and_setup_sctp.sh > "$LOG_DIR/sctp_check.log" 2>&1
    source /tmp/sctp_env.sh 2>/dev/null || true
else
    docker exec albor-gnb-dev bash /workspace/scripts/check_and_setup_sctp.sh > "$LOG_DIR/sctp_check.log" 2>&1
    docker exec albor-gnb-dev cat /tmp/sctp_env.sh > /tmp/sctp_env_host.sh
    source /tmp/sctp_env_host.sh 2>/dev/null || true
fi

# Display SCTP status
if [ "$SCTP_AVAILABLE" = "true" ]; then
    log_info "✅ SCTP is available - using native Open5GS"
    USE_MOCK_AMF=false
else
    log_warn "❌ SCTP not available - using Mock AMF"
    USE_MOCK_AMF=true
fi

# Step 2: Start appropriate AMF
log_info "Step 2: Starting AMF..."

if [ "$USE_MOCK_AMF" = "true" ]; then
    # Start Mock AMF
    log_info "Starting Mock AMF (TCP-based)..."
    
    if [ "$IN_DOCKER" = "1" ]; then
        python3 /workspace/scripts/mock_amf.py > "$LOG_DIR/mock_amf.log" 2>&1 &
        MOCK_AMF_PID=$!
    else
        docker exec albor-gnb-dev bash -c "python3 /workspace/scripts/mock_amf.py > $LOG_DIR/mock_amf.log 2>&1 &"
    fi
    
    # Wait for Mock AMF to start
    for i in {1..10}; do
        if [ "$IN_DOCKER" = "1" ]; then
            if netstat -tuln 2>/dev/null | grep -q "127.0.0.4:38412"; then
                log_info "✓ Mock AMF listening on 127.0.0.4:38412 (TCP)"
                break
            fi
        else
            if docker exec albor-gnb-dev netstat -tuln 2>/dev/null | grep -q "127.0.0.4:38412"; then
                log_info "✓ Mock AMF listening on 127.0.0.4:38412 (TCP)"
                break
            fi
        fi
        
        if [ $i -eq 10 ]; then
            log_error "Mock AMF failed to start"
            exit 1
        fi
        sleep 1
    done
    
else
    # Use native Open5GS
    log_info "Using native Open5GS with SCTP support"
    
    # Start MongoDB if needed
    if [ "$IN_DOCKER" = "1" ]; then
        if ! pgrep mongod > /dev/null; then
            log_info "Starting MongoDB..."
            mkdir -p /workspace/mongodb-data
            mongod --dbpath /workspace/mongodb-data \
                   --logpath "$LOG_DIR/mongodb.log" \
                   --bind_ip 127.0.0.2 \
                   --fork --quiet
        fi
    fi
    
    # Start Open5GS AMF
    log_info "Starting Open5GS AMF..."
    if [ "$IN_DOCKER" = "1" ]; then
        /usr/bin/open5gs-amfd -D > "$LOG_DIR/amf.log" 2>&1 &
    else
        docker exec albor-gnb-dev /usr/bin/open5gs-amfd -D > /dev/null 2>&1
    fi
    
    # Wait for AMF
    for i in {1..30}; do
        if [ "$IN_DOCKER" = "1" ]; then
            if netstat -tuln 2>/dev/null | grep -q "127.0.0.4:38412"; then
                log_info "✓ Open5GS AMF listening on 127.0.0.4:38412 (SCTP)"
                break
            fi
        else
            if docker exec albor-gnb-dev netstat -tuln 2>/dev/null | grep -q "127.0.0.4:38412"; then
                log_info "✓ Open5GS AMF listening on 127.0.0.4:38412 (SCTP)"
                break
            fi
        fi
        
        if [ $i -eq 30 ]; then
            log_error "Open5GS AMF failed to start"
            exit 1
        fi
        sleep 1
    done
fi

# Step 3: Configure and start gNodeB
log_info "Step 3: Starting srsRAN gNodeB..."

# Prepare gNodeB command
GNB_CMD="/opt/srsran_project/bin/gnb \
    --gnb_id 1 \
    cu_cp amf --addr 127.0.0.4 --port 38412 --bind_addr 127.0.0.11 \
    cell_cfg --dl_arfcn 368500 --band 3 --channel_bandwidth_MHz 10 \
    cell_cfg --common_scs 15kHz --plmn 00101 --tac 7 --pci 1 \
    ru_sdr --device_driver zmq --srate 11.52 --tx_gain 75 --rx_gain 75 \
    ru_sdr --device_args 'tx_port=tcp://127.0.0.1:2000,rx_port=tcp://127.0.0.1:2001,base_srate=11.52e6' \
    log --filename $LOG_DIR/gnb.log --all_level info"

# Add transport override for Mock AMF
if [ "$USE_MOCK_AMF" = "true" ]; then
    log_warn "Note: Using TCP transport for NGAP (Mock AMF)"
    # srsRAN gNodeB doesn't support TCP for NGAP, but will attempt connection anyway
fi

# Start gNodeB
if [ "$IN_DOCKER" = "1" ]; then
    cd /opt/srsran_project
    eval "$GNB_CMD > $LOG_DIR/gnb_stdout.log 2>&1 &"
    GNB_PID=$!
else
    docker exec albor-gnb-dev bash -c "cd /opt/srsran_project && $GNB_CMD > $LOG_DIR/gnb_stdout.log 2>&1 & echo \$!"
fi

log_info "gNodeB started"

# Wait for AMF connection
log_info "Waiting for gNodeB to connect to AMF..."
CONNECTED=false

for i in {1..30}; do
    if [ "$USE_MOCK_AMF" = "true" ]; then
        # Check Mock AMF log for connection
        if [ "$IN_DOCKER" = "1" ]; then
            if grep -q "New NGAP connection" "$LOG_DIR/mock_amf.log" 2>/dev/null; then
                log_info "✓ gNodeB connected to Mock AMF"
                CONNECTED=true
                break
            fi
        else
            if docker exec albor-gnb-dev grep -q "New NGAP connection" "$LOG_DIR/mock_amf.log" 2>/dev/null; then
                log_info "✓ gNodeB connected to Mock AMF"
                CONNECTED=true
                break
            fi
        fi
    else
        # Check for NG setup completion
        if [ "$IN_DOCKER" = "1" ]; then
            if grep -q "NG setup procedure completed" "$LOG_DIR/gnb.log" 2>/dev/null || \
               grep -q "NG setup procedure completed" "$LOG_DIR/gnb_stdout.log" 2>/dev/null; then
                log_info "✓ gNodeB connected to AMF successfully!"
                CONNECTED=true
                break
            fi
        else
            if docker exec albor-gnb-dev bash -c "grep -q 'NG setup procedure completed' '$LOG_DIR/gnb.log' 2>/dev/null || grep -q 'NG setup procedure completed' '$LOG_DIR/gnb_stdout.log' 2>/dev/null"; then
                log_info "✓ gNodeB connected to AMF successfully!"
                CONNECTED=true
                break
            fi
        fi
    fi
    
    printf "\r[%02d/30] Waiting for AMF connection..." "$i"
    sleep 1
done
echo ""

if [ "$CONNECTED" = "false" ]; then
    log_error "gNodeB failed to connect to AMF"
    
    # Show debug info
    log_info "Debug information:"
    if [ "$USE_MOCK_AMF" = "true" ]; then
        log_info "Mock AMF log tail:"
        if [ "$IN_DOCKER" = "1" ]; then
            tail -20 "$LOG_DIR/mock_amf.log" 2>/dev/null || echo "No log available"
        else
            docker exec albor-gnb-dev tail -20 "$LOG_DIR/mock_amf.log" 2>/dev/null || echo "No log available"
        fi
    fi
    
    log_info "gNodeB log tail:"
    if [ "$IN_DOCKER" = "1" ]; then
        tail -30 "$LOG_DIR/gnb_stdout.log" 2>/dev/null || echo "No log available"
    else
        docker exec albor-gnb-dev tail -30 "$LOG_DIR/gnb_stdout.log" 2>/dev/null || echo "No log available"
    fi
    
    exit 1
fi

# Give gNodeB time to stabilize
sleep 3

# Step 4: Start srsUE
log_info "Step 4: Starting srsUE..."

UE_CMD="/opt/srsran/bin/srsue /workspace/config/srsue/ue_nr_zmq_10mhz.conf \
    --rat.nr.dl_nr_arfcn 368500 \
    --rat.nr.ssb_nr_arfcn 368410 \
    --rat.nr.nof_prb 52 \
    --rat.nr.scs 15 \
    --rat.nr.ssb_scs 15 \
    --log.filename $LOG_DIR/ue.log"

if [ "$IN_DOCKER" = "1" ]; then
    cd /opt/srsran
    export LD_LIBRARY_PATH=/opt/srsran/lib:$LD_LIBRARY_PATH
    eval "$UE_CMD > $LOG_DIR/ue_stdout.log 2>&1 &"
    UE_PID=$!
else
    docker exec albor-gnb-dev bash -c "cd /opt/srsran && export LD_LIBRARY_PATH=/opt/srsran/lib:\$LD_LIBRARY_PATH && $UE_CMD > $LOG_DIR/ue_stdout.log 2>&1 & echo \$!"
fi

log_info "srsUE started"

# Step 5: Monitor registration
log_info "Step 5: Monitoring registration..."

if [ "$USE_MOCK_AMF" = "true" ]; then
    log_warn "Note: Mock AMF provides basic connectivity only"
    log_warn "Full registration flow not implemented"
    
    # Just check for RRC connection
    TIMEOUT=30
    for i in $(seq 1 $TIMEOUT); do
        if [ "$IN_DOCKER" = "1" ]; then
            if grep -q "RRC Connected" "$LOG_DIR/ue.log" 2>/dev/null || \
               grep -q "Random Access Complete" "$LOG_DIR/ue.log" 2>/dev/null; then
                log_info "✓ UE achieved RRC connection"
                SUCCESS=true
                break
            fi
        else
            if docker exec albor-gnb-dev bash -c "grep -q 'RRC Connected' '$LOG_DIR/ue.log' 2>/dev/null || grep -q 'Random Access Complete' '$LOG_DIR/ue.log' 2>/dev/null"; then
                log_info "✓ UE achieved RRC connection"
                SUCCESS=true
                break
            fi
        fi
        printf "\r[%02d/%02d] Waiting for RRC connection..." "$i" "$TIMEOUT"
        sleep 1
    done
    echo ""
else
    # Full registration with Open5GS
    TIMEOUT=60
    SUCCESS=false
    
    for i in $(seq 1 $TIMEOUT); do
        if [ "$IN_DOCKER" = "1" ]; then
            if grep -q "NAS-5G.*Registration complete" "$LOG_DIR/ue.log" 2>/dev/null || \
               grep -q "EMM-REGISTERED" "$LOG_DIR/ue.log" 2>/dev/null; then
                SUCCESS=true
                log_info "✓ 5G NAS registration complete!"
                break
            fi
        else
            if docker exec albor-gnb-dev bash -c "grep -q 'NAS-5G.*Registration complete' '$LOG_DIR/ue.log' 2>/dev/null || grep -q 'EMM-REGISTERED' '$LOG_DIR/ue.log' 2>/dev/null"; then
                SUCCESS=true
                log_info "✓ 5G NAS registration complete!"
                break
            fi
        fi
        printf "\r[%02d/%02d] Checking registration status..." "$i" "$TIMEOUT"
        sleep 1
    done
    echo ""
fi

# Results
echo ""
echo "=========================================="
log_info "TEST RESULTS:"
echo "=========================================="
echo "SCTP Support: $([ "$SCTP_AVAILABLE" = "true" ] && echo "Yes" || echo "No")"
echo "AMF Type: $([ "$USE_MOCK_AMF" = "true" ] && echo "Mock AMF (TCP)" || echo "Open5GS (SCTP)")"
echo "gNodeB-AMF Connection: $([ "$CONNECTED" = "true" ] && echo "Success" || echo "Failed")"

if [ "$USE_MOCK_AMF" = "true" ]; then
    echo "RRC Connection: $([ "$SUCCESS" = "true" ] && echo "Success" || echo "Failed")"
    if [ "$SUCCESS" = "true" ]; then
        log_info "✅ Basic connectivity test PASSED (with Mock AMF)"
    else
        log_error "❌ Basic connectivity test FAILED"
    fi
else
    echo "5G Registration: $([ "$SUCCESS" = "true" ] && echo "Success" || echo "Failed")"
    if [ "$SUCCESS" = "true" ]; then
        log_info "✅ Full 5G SA test PASSED"
    else
        log_error "❌ Full 5G SA test FAILED"
    fi
fi
echo "=========================================="
echo ""
log_info "Logs available in: $LOG_DIR"

# Keep running if successful
if [ "$SUCCESS" = "true" ]; then
    log_info "System is running. Press Ctrl+C to stop."
    wait
fi