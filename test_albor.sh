#!/bin/bash
# test_albor.sh - Complete 5G SA test with Albor gNodeB + srsUE + Open5GS
# Tests our implementation against the reference UE with full 5G core

set -e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

log_info() { echo -e "${GREEN}[INFO]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }
log_warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }

# Create log directory
LOG_DIR="/tmp/logs/$(date +%Y%m%d_%H%M%S)_albor_full"
mkdir -p "$LOG_DIR"

log_info "=== Albor Complete 5G SA Test ==="
log_info "Testing: Open5GS + Albor gNodeB + srsRAN UE"
log_info "Log directory: $LOG_DIR"

# Cleanup function
cleanup() {
    log_info "Cleaning up..."
    # Kill UE and gNodeB
    if [ "$IN_DOCKER" = "1" ]; then
        pkill -f 'albor_gnodeb|srsue' || true
    else
        docker exec albor-gnb-dev bash -c "pkill -f 'albor_gnodeb|srsue' || true" 2>/dev/null || true
    fi
    # Kill processes on ZMQ ports
    for port in 2000 2001; do
        PID=$(lsof -ti:$port 2>/dev/null || true)
        if [ ! -z "$PID" ]; then
            kill -9 $PID 2>/dev/null || true
        fi
    done
}
trap cleanup EXIT

# Step 1: Start Open5GS Core Network
log_info "Step 1: Starting Open5GS Core Network (Native Installation)..."

# Check if we're running inside the container
if [ -f /.dockerenv ]; then
    IN_DOCKER=1
    log_info "Running inside Docker container"
else
    log_info "Running test through Docker exec"
fi

# Start Open5GS using native installation
if [ "$IN_DOCKER" = "1" ]; then
    # Inside container - run directly
    /workspace/scripts/open5gs/start_open5gs_core.sh
else
    # Outside container - use docker exec
    docker exec albor-gnb-dev /workspace/scripts/open5gs/start_open5gs_core.sh
fi

# Wait for AMF to be ready on loopback interface
log_info "Verifying Open5GS AMF is ready..."
for i in {1..30}; do
    if [ "$IN_DOCKER" = "1" ]; then
        if netstat -tuln 2>/dev/null | grep -q "127.0.0.4:38412"; then
            log_info "✓ AMF is listening on 127.0.0.4:38412"
            break
        fi
    else
        if docker exec albor-gnb-dev netstat -tuln 2>/dev/null | grep -q "127.0.0.4:38412"; then
            log_info "✓ AMF is listening on 127.0.0.4:38412"
            break
        fi
    fi
    
    if [ $i -eq 30 ]; then
        log_error "AMF failed to start on 127.0.0.4:38412"
        exit 1
    fi
    
    printf "\r[%02d/30] Waiting for AMF to be ready..." "$i"
    sleep 1
done
echo ""

# Kill existing processes
log_info "Stopping any existing processes..."
cleanup
sleep 2

# Step 2: Build and configure Albor gNodeB
log_info "Step 2: Building and starting Albor gNodeB..."

# AMF is on loopback interface with multi-loopback setup
AMF_IP="127.0.0.4"
log_info "AMF IP address: $AMF_IP"

# Create log directory
if [ "$IN_DOCKER" = "1" ]; then
    mkdir -p "$LOG_DIR"
else
    docker exec albor-gnb-dev mkdir -p "$LOG_DIR"
    
    # Ensure albor-gnb-dev container is running
    if ! docker ps | grep -q albor-gnb-dev; then
        log_error "albor-gnb-dev container is not running. Please start it first."
        exit 1
    fi
fi

# Build Albor gNodeB if needed
if [ "$IN_DOCKER" = "1" ]; then
    cd /workspace
    if [ ! -f target/release/albor_gnodeb ]; then
        echo 'Building Albor gNodeB...'
        cargo build --release > $LOG_DIR/build.log 2>&1
        if [ $? -ne 0 ]; then
            echo 'Build failed'
            tail -20 $LOG_DIR/build.log
            exit 1
        fi
    fi
else
    docker exec albor-gnb-dev bash -c "
    cd /workspace
    if [ ! -f target/release/albor_gnodeb ]; then
        echo 'Building Albor gNodeB...'
        cargo build --release > $LOG_DIR/build.log 2>&1
        if [ \$? -ne 0 ]; then
            echo 'Build failed'
            tail -20 $LOG_DIR/build.log
            exit 1
        fi
    fi
    "
fi

# Start Albor gNodeB with sacred configuration file
if [ "$IN_DOCKER" = "1" ]; then
    cd /workspace
    mkdir -p $LOG_DIR
    ./target/release/albor_gnodeb -c /workspace/config/albor_gnb/gnb_albor.yml > $LOG_DIR/gnb.log 2>&1 &
    GNB_PID=$!
    echo $GNB_PID > /tmp/gnb_pid.txt
else
    docker exec albor-gnb-dev bash -c "
    cd /workspace
    mkdir -p $LOG_DIR
    ./target/release/albor_gnodeb -c /workspace/config/albor_gnb/gnb_albor.yml > $LOG_DIR/gnb.log 2>&1 &
    echo \$!
    " > /tmp/gnb_pid.txt
    GNB_PID=$(cat /tmp/gnb_pid.txt)
fi

log_info "Albor gNodeB started (PID: $GNB_PID)"

# Wait for AMF connection
log_info "Waiting for Albor gNodeB to connect to AMF..."
for i in {1..30}; do
    if [ "$IN_DOCKER" = "1" ]; then
        if grep -q "Connected to AMF\|NGAP.*established" "$LOG_DIR/gnb.log" 2>/dev/null; then
            log_info "✓ Albor gNodeB connected to AMF successfully!"
            break
        fi
    else
        if docker exec albor-gnb-dev grep -q "Connected to AMF\|NGAP.*established" "$LOG_DIR/gnb.log" 2>/dev/null; then
            log_info "✓ Albor gNodeB connected to AMF successfully!"
            break
        fi
    fi
    if [ $i -eq 30 ]; then
        log_warn "⚠ Albor gNodeB may not have connected to AMF yet"
        if [ "$IN_DOCKER" = "1" ]; then
            tail -20 "$LOG_DIR/gnb.log"
        else
            docker exec albor-gnb-dev tail -20 "$LOG_DIR/gnb.log"
        fi
    fi
    sleep 1
done

# Step 3: Start srsUE
log_info "Step 3: Starting srsUE..."

if [ "$IN_DOCKER" = "1" ]; then
    cd /opt/srsran
    export LD_LIBRARY_PATH=/opt/srsran/lib:$LD_LIBRARY_PATH
    mkdir -p $LOG_DIR
    /opt/srsran/bin/srsue /workspace/config/srsue/ue_nr_zmq_10mhz.conf > $LOG_DIR/ue.log 2>&1 &
    UE_PID=$!
    echo $UE_PID > /tmp/ue_pid.txt
else
    docker exec albor-gnb-dev bash -c "
    cd /opt/srsran
    export LD_LIBRARY_PATH=/opt/srsran/lib:\$LD_LIBRARY_PATH
    mkdir -p $LOG_DIR
    /opt/srsran/bin/srsue /workspace/config/srsue/ue_nr_zmq_10mhz.conf > $LOG_DIR/ue.log 2>&1 &
    echo \$!
    " > /tmp/ue_pid.txt
    UE_PID=$(cat /tmp/ue_pid.txt)
fi

log_info "srsUE started (PID: $UE_PID)"

# Step 4: Monitor registration
log_info "Step 4: Monitoring 5G registration..."

TIMEOUT=60
SUCCESS=false

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

# Helper function to grep and pipe
grep_pipe() {
    local pattern1="$1"
    local pattern2="$2"
    local file="$3"
    if [ "$IN_DOCKER" = "1" ]; then
        grep "$pattern1" "$file" 2>/dev/null | grep -q "$pattern2"
    else
        docker exec albor-gnb-dev sh -c "grep '$pattern1' '$file' 2>/dev/null | grep -q '$pattern2'"
    fi
}

for i in $(seq 1 $TIMEOUT); do
    printf "\r[%02d/%02d] Checking registration status..." "$i" "$TIMEOUT"
    
    # Check various stages
    if check_log "Found Cell" "$LOG_DIR/ue.log"; then
        if ! [ "$CELL_FOUND" = "1" ]; then
            echo ""
            log_info "✓ UE found cell"
            CELL_FOUND=1
        fi
    fi
    
    if check_log "Random Access Complete" "$LOG_DIR/ue.log"; then
        if ! [ "$RACH_COMPLETE" = "1" ]; then
            echo ""
            log_info "✓ Random access completed"
            RACH_COMPLETE=1
        fi
    fi
    
    if check_log "RRC Connected" "$LOG_DIR/ue.log"; then
        if ! [ "$RRC_CONNECTED" = "1" ]; then
            echo ""
            log_info "✓ RRC connected"
            RRC_CONNECTED=1
        fi
    fi
    
    if grep_pipe "NAS" "EMM-REGISTERED" "$LOG_DIR/ue.log"; then
        SUCCESS=true
        break
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
        tail -$lines "$file" 2>/dev/null || echo "  (log file not found)"
    else
        docker exec albor-gnb-dev tail -$lines "$file" 2>/dev/null || echo "  (log file not found)"
    fi
}

# Helper function to grep logs
grep_log() {
    local pattern="$1"
    local file="$2"
    local lines="${3:-20}"
    if [ "$IN_DOCKER" = "1" ]; then
        grep -E "$pattern" "$file" 2>/dev/null | tail -$lines || echo "  (no matches found)"
    else
        docker exec albor-gnb-dev sh -c "grep -E '$pattern' '$file' 2>/dev/null | tail -$lines" || echo "  (no matches found)"
    fi
}

if [ "$SUCCESS" = "true" ]; then
    log_info "✅ SUCCESS: UE registered to 5G network via Albor gNodeB!"
    
    # Show key milestones
    grep_log "(Found Cell|Random Access|RRC Connected|EMM-REGISTERED|PDU Session)" "$LOG_DIR/ue.log" 20
    
    # Check AMF logs
    echo ""
    log_info "AMF status:"
    grep_log "(Registered|PDU|Session|InitialUEMessage)" "/var/log/open5gs/amf.log" 5
else
    log_error "❌ FAILED: UE did not register"
    
    # Debug info
    echo ""
    log_info "Albor gNodeB log tail:"
    show_log "$LOG_DIR/gnb.log" 30
    
    echo ""
    log_info "UE log tail:"
    show_log "$LOG_DIR/ue.log" 30
    
    echo ""
    log_info "AMF log tail:"
    show_log "/var/log/open5gs/amf.log" 20
    echo ""
    log_info "SMF log tail:"
    show_log "/var/log/open5gs/smf.log" 10
fi

echo "=========================================="

# Keep running if successful
if [ "$SUCCESS" = "true" ]; then
    log_info "System is running. Press Ctrl+C to stop."
    wait
fi