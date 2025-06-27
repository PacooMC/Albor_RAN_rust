#!/bin/bash
# Quick srsRAN test - no waiting for confirmations

set +e

# Colors
GREEN='\033[0;32m'
RED='\033[0;31m'
NC='\033[0m'

log_info() { echo -e "${GREEN}[INFO]${NC} $1"; }

# Create log directory
LOG_DIR="logs/$(date +%Y%m%d_%H%M%S)_srsran_quick"
mkdir -p "$LOG_DIR"

CONTAINER_NAME="albor-gnb-dev"

# Cleanup function
cleanup() {
    log_info "Cleaning up..."
    docker compose exec -T $CONTAINER_NAME bash -c "pkill -9 -f 'gnb' 2>/dev/null || true"
    docker compose exec -T $CONTAINER_NAME bash -c "pkill -9 -f 'srsue' 2>/dev/null || true"
}

trap cleanup EXIT

log_info "=== Quick srsRAN Test ==="
log_info "Log directory: $LOG_DIR"

# Initial cleanup
cleanup

# Update config log paths
docker compose exec $CONTAINER_NAME bash -c "
    cp /workspace/config/srsran_gnb/gnb_zmq.yml /tmp/gnb_config.yml
    sed -i 's|/workspace/logs/|/workspace/$LOG_DIR/|g' /tmp/gnb_config.yml
"

# Start gNodeB
log_info "Starting srsRAN gNodeB..."
docker compose exec -d $CONTAINER_NAME bash -c "
    cd /workspace
    /usr/local/bin/gnb -c /tmp/gnb_config.yml > /workspace/$LOG_DIR/gnb_stdout.log 2>&1
"

# Wait 2 seconds for gNodeB to initialize
sleep 2

# Start srsUE
log_info "Starting srsUE..."
docker compose exec -d $CONTAINER_NAME bash -c "
    export LD_LIBRARY_PATH=/usr/local/lib:\$LD_LIBRARY_PATH
    /usr/local/bin/srsue /workspace/config/srsue/ue_zmq.conf > /workspace/$LOG_DIR/ue_stdout.log 2>&1
"

# Run for 8 seconds
log_info "Running test for 8 seconds..."
for i in {1..8}; do
    printf "\r[%d/8] Running..." "$i"
    sleep 1
done
echo ""

# Kill processes
log_info "Stopping test..."
cleanup

# Check logs
log_info "Checking results..."

echo ""
log_info "gNodeB stdout (last 20 lines):"
docker compose exec -T $CONTAINER_NAME tail -20 "$LOG_DIR/gnb_stdout.log" 2>/dev/null || echo "No stdout log"

echo ""
log_info "gNodeB log file (last 20 lines):"
docker compose exec -T $CONTAINER_NAME tail -20 "$LOG_DIR/gnb.log" 2>/dev/null || echo "No log file"

echo ""
log_info "UE log (last 20 lines):"
docker compose exec -T $CONTAINER_NAME tail -20 "$LOG_DIR/ue_stdout.log" 2>/dev/null || echo "No UE log"

echo ""
log_info "AMF log (checking for connections):"
docker compose exec -T $CONTAINER_NAME grep -E "(NG Setup|Initial|5GMM)" "/open5gs/install/var/log/open5gs/amf.log" 2>/dev/null | tail -10 || echo "No AMF activity"

log_info "Test complete. Logs saved in: $LOG_DIR"