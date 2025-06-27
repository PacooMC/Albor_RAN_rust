#!/bin/bash
# Test srsRAN with YAML config file

set +e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

log_info() { echo -e "${GREEN}[INFO]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }

# Create log directory
LOG_DIR="logs/$(date +%Y%m%d_%H%M%S)_srsran_yaml"
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

log_info "=== srsRAN 5G Test with YAML Config ==="
log_info "Log directory: $LOG_DIR"

# Initial cleanup
cleanup

# Update gnb_zmq.yml log paths
docker compose exec $CONTAINER_NAME bash -c "
    cp /workspace/config/srsran_gnb/gnb_zmq.yml /tmp/gnb_config.yml
    sed -i 's|/workspace/logs/gnb.log|/workspace/$LOG_DIR/gnb.log|g' /tmp/gnb_config.yml
    sed -i 's|/workspace/logs/gnb_mac.pcap|/workspace/$LOG_DIR/gnb_mac.pcap|g' /tmp/gnb_config.yml
    sed -i 's|/workspace/logs/gnb_ngap.pcap|/workspace/$LOG_DIR/gnb_ngap.pcap|g' /tmp/gnb_config.yml
"

# Start gNodeB with YAML config
log_info "Starting srsRAN gNodeB with YAML config..."
docker compose exec -d $CONTAINER_NAME bash -c "
    cd /workspace
    /usr/local/bin/gnb -c /tmp/gnb_config.yml > /workspace/$LOG_DIR/gnb_stdout.log 2>&1
"

# Wait for gNodeB to start
log_info "Waiting for gNodeB to connect to AMF..."
for i in {1..30}; do
    if docker compose exec -T $CONTAINER_NAME grep -q "NG setup procedure completed" "$LOG_DIR/gnb.log" 2>/dev/null || \
       docker compose exec -T $CONTAINER_NAME grep -q "Connected to AMF" "$LOG_DIR/gnb_stdout.log" 2>/dev/null || \
       docker compose exec -T $CONTAINER_NAME grep -q "Completed AMF connection" "$LOG_DIR/gnb_stdout.log" 2>/dev/null; then
        log_info "✓ gNodeB connected to AMF!"
        break
    fi
    
    if [ $i -eq 30 ]; then
        log_error "gNodeB failed to connect to AMF"
        echo "gNodeB stdout log:"
        docker compose exec -T $CONTAINER_NAME tail -30 "$LOG_DIR/gnb_stdout.log" 2>/dev/null || echo "No stdout log"
        echo ""
        echo "gNodeB log file:"
        docker compose exec -T $CONTAINER_NAME tail -30 "$LOG_DIR/gnb.log" 2>/dev/null || echo "No log file"
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
    /usr/local/bin/srsue /workspace/config/srsue/ue_zmq.conf > /workspace/$LOG_DIR/ue_stdout.log 2>&1
"

# Monitor for registration
log_info "Monitoring UE registration..."
SUCCESS=false
CELL_FOUND=false
RRC_CONNECTED=false

for i in {1..60}; do
    printf "\r[%02d/60] Checking registration..." "$i"
    
    # Check for cell found
    if docker compose exec -T $CONTAINER_NAME grep -q "Found Cell" "$LOG_DIR/ue_stdout.log" 2>/dev/null; then
        if [ "$CELL_FOUND" = "false" ]; then
            echo ""
            log_info "✓ UE found cell"
            CELL_FOUND=true
        fi
    fi
    
    # Check for RRC connection
    if docker compose exec -T $CONTAINER_NAME grep -q "RRC Connected" "$LOG_DIR/ue_stdout.log" 2>/dev/null; then
        if [ "$RRC_CONNECTED" = "false" ]; then
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
    log_info "Registration milestones:"
    [ "$CELL_FOUND" = "true" ] && echo "  ✓ Cell detection"
    [ "$RRC_CONNECTED" = "true" ] && echo "  ✓ RRC connection"
    echo "  ✓ 5G NAS registration"
else
    log_error "❌ FAILED: UE did not register"
    
    echo ""
    log_info "Registration milestones:"
    [ "$CELL_FOUND" = "true" ] && echo "  ✓ Cell detection" || echo "  ✗ Cell detection"
    [ "$RRC_CONNECTED" = "true" ] && echo "  ✓ RRC connection" || echo "  ✗ RRC connection"
    echo "  ✗ 5G NAS registration"
    
    echo ""
    log_info "gNodeB stdout log tail:"
    docker compose exec -T $CONTAINER_NAME tail -30 "$LOG_DIR/gnb_stdout.log" 2>/dev/null || echo "No stdout log"
    
    echo ""
    log_info "gNodeB log file tail:"
    docker compose exec -T $CONTAINER_NAME tail -30 "$LOG_DIR/gnb.log" 2>/dev/null || echo "No log file"
    
    echo ""
    log_info "UE log tail:"
    docker compose exec -T $CONTAINER_NAME tail -30 "$LOG_DIR/ue_stdout.log" 2>/dev/null || echo "No log"
    
    echo ""
    log_info "AMF log tail (last registration attempts):"
    docker compose exec -T $CONTAINER_NAME grep -E "(Initial|Registration|5GMM)" "/var/log/open5gs/amf.log" 2>/dev/null | tail -10 || echo "No AMF logs"
fi

echo "=========================================="

# If successful, keep running for monitoring
if [ "$SUCCESS" = "true" ]; then
    log_info "System is running. Press Ctrl+C to stop."
    wait
fi