#!/bin/bash
# test_5g_sa_loopback.sh - Complete 5G SA test with loopback interfaces
# This solves the GTP-U port conflict by using different IPs

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
LOG_DIR="/workspace/logs/$(date +%Y%m%d_%H%M%S)_loopback_test"
mkdir -p "$LOG_DIR"

log_info "=== 5G SA Test with Loopback Interfaces ==="
log_info "Log directory: $LOG_DIR"

# Cleanup function
cleanup() {
    log_info "Cleaning up..."
    # Stop all processes
    pkill -f 'gnb|srsue|open5gs-' || true
    # Kill processes on ZMQ ports
    for port in 2000 2001; do
        PID=$(lsof -ti:$port 2>/dev/null || true)
        if [ ! -z "$PID" ]; then
            kill -9 $PID 2>/dev/null || true
        fi
    done
    # Stop MongoDB
    pkill -f mongod || true
}
trap cleanup EXIT

# Check if we're in the Docker container
if [ ! -f /.dockerenv ]; then
    log_error "This script must be run inside the albor-gnb-dev Docker container"
    exit 1
fi

# Step 1: Setup loopback interfaces
log_info "Step 1: Setting up loopback interfaces..."
if ! ip addr show lo2 &>/dev/null; then
    log_info "Creating loopback interfaces..."
    /workspace/setup_network_loopback.sh
else
    log_info "Loopback interfaces already exist"
fi

# Step 2: Start Open5GS
log_info "Step 2: Starting Open5GS Core Network..."
cd /workspace/config/open5gs_native

# Stop any existing Open5GS
pkill -f open5gs- || true
pkill -f mongod || true
sleep 2

# Start Open5GS
./start_open5gs.sh > "$LOG_DIR/open5gs_startup.log" 2>&1 &
OPEN5GS_PID=$!

# Wait for Open5GS to be ready
log_info "Waiting for Open5GS to initialize..."
for i in {1..30}; do
    if grep -q "All Open5GS services started successfully" "$LOG_DIR/open5gs_startup.log" 2>/dev/null; then
        log_info "✓ Open5GS is ready!"
        break
    fi
    if [ $i -eq 30 ]; then
        log_error "Open5GS failed to start"
        cat "$LOG_DIR/open5gs_startup.log"
        exit 1
    fi
    sleep 1
done

# Additional wait for stability
sleep 5

# Step 3: Start gNodeB
log_info "Step 3: Starting srsRAN gNodeB..."
cd /opt/srsran_project

# Kill any existing gNodeB
pkill -f gnb || true
sleep 2

# Start gNodeB with loopback config
log_info "Starting gNodeB on 127.0.0.11..."
/opt/srsran_project/bin/gnb -c /workspace/config/srsran_gnb/gnb_zmq_loopback.yml > "$LOG_DIR/gnb.log" 2>&1 &
GNB_PID=$!

# Wait for AMF connection
log_info "Waiting for gNodeB to connect to AMF..."
for i in {1..30}; do
    if grep -q "NG setup procedure completed" "$LOG_DIR/gnb.log" 2>/dev/null; then
        log_info "✓ gNodeB connected to AMF successfully!"
        break
    fi
    if grep -q "Failed to bind" "$LOG_DIR/gnb.log" 2>/dev/null; then
        log_error "gNodeB failed to bind to port"
        tail -20 "$LOG_DIR/gnb.log"
        exit 1
    fi
    if [ $i -eq 30 ]; then
        log_error "gNodeB failed to connect to AMF"
        tail -20 "$LOG_DIR/gnb.log"
        exit 1
    fi
    sleep 1
done

# Step 4: Start UE
log_info "Step 4: Starting srsUE..."
cd /opt/srsran

# Kill any existing UE
pkill -f srsue || true
sleep 2

export LD_LIBRARY_PATH=/opt/srsran/lib:$LD_LIBRARY_PATH
/opt/srsran/bin/srsue /workspace/config/srsue/ue_nr_zmq.conf > "$LOG_DIR/ue.log" 2>&1 &
UE_PID=$!

# Step 5: Monitor registration
log_info "Step 5: Monitoring 5G registration..."

TIMEOUT=60
SUCCESS=false

for i in $(seq 1 $TIMEOUT); do
    printf "\r[%02d/%02d] Checking registration status..." "$i" "$TIMEOUT"
    
    # Check various stages
    if grep -q "Found Cell" "$LOG_DIR/ue.log" 2>/dev/null; then
        if ! [ "$CELL_FOUND" = "1" ]; then
            echo ""
            log_info "✓ UE found cell"
            CELL_FOUND=1
        fi
    fi
    
    if grep -q "Random Access Complete" "$LOG_DIR/ue.log" 2>/dev/null; then
        if ! [ "$RACH_COMPLETE" = "1" ]; then
            echo ""
            log_info "✓ Random access completed"
            RACH_COMPLETE=1
        fi
    fi
    
    if grep -q "RRC Connected" "$LOG_DIR/ue.log" 2>/dev/null; then
        if ! [ "$RRC_CONNECTED" = "1" ]; then
            echo ""
            log_info "✓ RRC connected"
            RRC_CONNECTED=1
        fi
    fi
    
    if grep -q "NAS.*EMM-REGISTERED" "$LOG_DIR/ue.log" 2>/dev/null; then
        echo ""
        log_info "✓ NAS registration completed!"
        SUCCESS=true
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
    
    # Show network status
    echo ""
    log_info "Network Status:"
    
    # Check GTP-U endpoints
    log_info "GTP-U Endpoints:"
    ss -unp | grep 2152 | while read line; do
        echo "  $line"
    done
    
    # Show key UE milestones
    echo ""
    log_info "UE Registration Milestones:"
    grep -E "(Found Cell|Random Access|RRC Connected|EMM-REGISTERED|PDU Session)" "$LOG_DIR/ue.log" | tail -10
    
    # Check AMF logs
    echo ""
    log_info "AMF Registration Status:"
    tail -5 "$LOG_DIR/open5gs_startup.log" | grep -E "(Initial UE|Registration|PDU)" || true
    tail -10 /workspace/logs/open5gs/amf.log | grep -E "(Initial UE|Registration|PDU)" || true
    
    # Test data connectivity
    echo ""
    log_info "Testing data connectivity..."
    # Check if UE got IP address
    if grep -q "PDU Session Establishment successful" "$LOG_DIR/ue.log"; then
        log_info "✓ PDU session established"
        # Try to find assigned IP
        UE_IP=$(grep -oP "Assigned IP: \K[0-9.]+|IP address: \K[0-9.]+" "$LOG_DIR/ue.log" || echo "")
        if [ ! -z "$UE_IP" ]; then
            log_info "✓ UE IP address: $UE_IP"
        fi
    fi
else
    log_error "❌ FAILED: UE did not register"
    
    # Debug info
    echo ""
    log_info "gNodeB log tail:"
    tail -20 "$LOG_DIR/gnb.log"
    
    echo ""
    log_info "UE log tail:"
    tail -20 "$LOG_DIR/ue.log"
    
    echo ""
    log_info "AMF log tail:"
    tail -20 /workspace/logs/open5gs/amf.log 2>/dev/null || true
    
    echo ""
    log_info "GTP-U port status:"
    ss -unp | grep 2152 || echo "No GTP-U listeners found"
fi

echo "=========================================="

# Show port bindings
echo ""
log_info "Port Bindings:"
log_info "AMF NGAP: $(ss -anp | grep :38412 | head -1)"
log_info "UPF GTP-U: $(ss -unp | grep 127.0.0.10:2152 | head -1)"
log_info "gNB GTP-U: $(ss -unp | grep 127.0.0.11:2152 | head -1)"

# Keep running if successful
if [ "$SUCCESS" = "true" ]; then
    log_info ""
    log_info "System is running successfully with loopback isolation!"
    log_info "Press Ctrl+C to stop."
    wait
fi