#!/bin/bash
# Albor Space 5G GNodeB Quick Test Script
# This script validates the GNodeB implementation with srsRAN Project 5G UE
# MUST RUN INSIDE DOCKER CONTAINER
#
# Usage: ./quicktest.sh [--use-reference]
#   --use-reference: Use srsRAN gNodeB instead of our implementation for reference testing
#
# CRITICAL: This script checks if Docker container is running and uses it without restart

set -e  # Exit on error

# Parse command line arguments
USE_REFERENCE=false
while [[ $# -gt 0 ]]; do
    case $1 in
        --use-reference)
            USE_REFERENCE=true
            shift
            ;;
        *)
            echo "Unknown option: $1"
            echo "Usage: $0 [--use-reference]"
            exit 1
            ;;
    esac
done

# Color codes for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Logging functions
log_info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

log_debug() {
    echo -e "${BLUE}[DEBUG]${NC} $1"
}

# Check if we're running inside Docker container
if [ -f /.dockerenv ]; then
    log_info "Running inside Docker container (good!)"
    # We're inside the container, proceed normally
    
    # Create dated log directory
    LOG_DIR="logs/$(date +%Y%m%d_%H%M%S)"
    mkdir -p "$LOG_DIR"
    log_info "Created log directory: $LOG_DIR"
    
    # Clean up any processes using ZMQ ports
    log_info "Cleaning up ZMQ ports 2000 and 2001..."
    
    # Kill any existing srsue or gnb processes
    killall -9 srsue 2>/dev/null || true
    killall -9 gnb 2>/dev/null || true
    killall -9 albor_gnodeb 2>/dev/null || true
    
    # Check ports using ss (more portable than lsof)
    # Port 2000
    PORT_2000_PID=$(ss -tlnp 2>/dev/null | grep ':2000' | grep -oE 'pid=[0-9]+' | cut -d= -f2 | head -1)
    if [ ! -z "$PORT_2000_PID" ]; then
        log_warn "Port 2000 is in use by PID $PORT_2000_PID. Terminating..."
        kill -9 $PORT_2000_PID 2>/dev/null || true
    fi
    
    # Port 2001
    PORT_2001_PID=$(ss -tlnp 2>/dev/null | grep ':2001' | grep -oE 'pid=[0-9]+' | cut -d= -f2 | head -1)
    if [ ! -z "$PORT_2001_PID" ]; then
        log_warn "Port 2001 is in use by PID $PORT_2001_PID. Terminating..."
        kill -9 $PORT_2001_PID 2>/dev/null || true
    fi
    
    # Wait a bit for ports to be released
    sleep 2
    
    log_info "ZMQ ports cleaned up"
    
    if [ "$USE_REFERENCE" = true ]; then
        log_info "Starting validation with REFERENCE srsRAN gNodeB..."
    else
        log_info "Starting Albor Space 5G GNodeB validation..."
    fi
    
    # Step 1: Compile our GNodeB project (always compile fresh)
    if [ "$USE_REFERENCE" = false ]; then
        log_info "Compiling Albor Space GNodeB project..."
        if [ -f "/workspace/Cargo.toml" ]; then
            cd /workspace
            cargo build --release 2>&1 | tee "$LOG_DIR/build.log"
            if [ ${PIPESTATUS[0]} -ne 0 ]; then
                log_error "Build failed! Check $LOG_DIR/build.log for details"
                exit 1
            fi
            log_info "Build completed successfully"
        else
            log_error "Cargo.toml not found at /workspace. Project not initialized."
            exit 1
        fi
    fi
    
    # Step 2: Execute GNodeB (our implementation or reference)
    if [ "$USE_REFERENCE" = true ]; then
        log_info "Starting REFERENCE srsRAN gNodeB..."
        
        # Create gNodeB configuration for reference
        cat > /tmp/gnb_zmq.conf << 'EOF'
amf:
  addr: 127.0.0.1
  bind_addr: 127.0.0.1

ru_sdr:
  device_driver: zmq
  device_args: tx_port=tcp://127.0.0.1:2000,rx_port=tcp://127.0.0.1:2001,base_srate=23.04e6
  srate: 23.04e6
  tx_gain: 75
  rx_gain: 75

cell_cfg:
  dl_arfcn: 368500
  band: 3
  channel_bandwidth_MHz: 20
  common_scs: 15
  plmn: "00101"
  tac: 1

log:
  filename: /tmp/gnb.log
  all_level: info
EOF
        
        # Check if srsRAN gNodeB exists (pre-compiled in Docker image)
        if command -v gnb >/dev/null 2>&1; then
            # Start gNodeB with continuous log tailing
            gnb /tmp/gnb_zmq.conf > "$LOG_DIR/reference_gnb.log" 2>&1 &
            GNODEB_PID=$!
            log_info "Reference gNodeB started with PID: $GNODEB_PID"
            
            # Start tailing logs in background
            tail -f "$LOG_DIR/reference_gnb.log" | sed 's/^/[GNB] /' &
            TAIL_GNB_PID=$!
        else
            log_error "srsRAN gNodeB not found. Docker image may not be built correctly."
            exit 1
        fi
    else
        log_info "Starting Albor Space GNodeB..."
        if [ -f "/workspace/target/release/albor_gnodeb" ]; then
            cd /workspace
            ./target/release/albor_gnodeb > "$LOG_DIR/gnodeb.log" 2>&1 &
            GNODEB_PID=$!
            log_info "Albor GNodeB started with PID: $GNODEB_PID"
            
            # Start tailing logs in background
            tail -f "$LOG_DIR/gnodeb.log" | sed 's/^/[GNB] /' &
            TAIL_GNB_PID=$!
        else
            log_error "Albor GNodeB binary not found. Build failed or not completed."
            exit 1
        fi
    fi
    
    # Wait for GNodeB to initialize
    sleep 2
    
    # Check if GNodeB is still running
    if ! kill -0 $GNODEB_PID 2>/dev/null; then
        if [ "$USE_REFERENCE" = true ]; then
            log_error "Reference gNodeB failed to start. Check $LOG_DIR/reference_gnb.log"
        else
            log_error "Albor GNodeB failed to start. Check $LOG_DIR/gnodeb.log"
        fi
        exit 1
    fi
    
    # Step 3: Create UE configuration for 5G NR with ZMQ
    log_info "Creating UE configuration for 5G NR..."
    cat > /tmp/ue_zmq.conf << 'EOF'
[rf]
freq_offset = 0
tx_gain = 50
rx_gain = 40
srate = 23.04e6
nof_antennas = 1

# ZMQ configuration for connection to GNodeB
device_name = zmq
device_args = fail_on_disconnect=true,tx_port0=tcp://*:2001,rx_port0=tcp://localhost:2000,base_srate=23.04e6

[rat.eutra]
dl_earfcn = 3350
nof_carriers = 0

[rat.nr]
bands = 3
nof_carriers = 1
max_nof_prb = 106
nof_prb = 106

[rrc]
release = 15
ue_category = 4
# Enable NR measurements
nr_measurement_pci = 1
nr_short_sn_support = true

[pcap]
enable = mac_nr
mac_filename = /tmp/ue_mac.pcap
mac_nr_filename = /tmp/ue_mac_nr.pcap
nas_filename = /tmp/ue_nas.pcap

[log]
all_level = debug
# Enable maximum PHY logging to debug cell search
phy_level = debug
phy_lib_level = debug
mac_level = debug
rlc_level = debug
rrc_level = debug
nas_level = debug
all_hex_limit = 64
filename = /tmp/ue.log
file_max_size = -1

[usim]
mode = soft
algo = milenage
opc  = 63BFA50EE6523365FF14C1F45F88737D
k    = 00112233445566778899aabbccddeeff
imsi = 001010123456789
imei = 353490069873319

[nas]
apn = internet
apn_protocol = ipv4

[gw]
netns = 
ip_devname = tun_srsue
ip_netmask = 255.255.255.0

[gui]
enable = false
EOF
    
    # Step 3: Execute srsRAN 5G UE (pre-compiled in Docker image)
    log_info "Starting srsRAN 5G UE..."
    if command -v srsue >/dev/null 2>&1; then
        # Set library path for srsue
        export LD_LIBRARY_PATH=/opt/srsran/lib:$LD_LIBRARY_PATH
        srsue /tmp/ue_zmq.conf > "$LOG_DIR/ue.log" 2>&1 &
        UE_PID=$!
        log_info "srsRAN 5G UE started with PID: $UE_PID"
        
        # Start tailing UE logs in background
        tail -f "$LOG_DIR/ue.log" | sed 's/^/[UE] /' &
        TAIL_UE_PID=$!
    else
        log_error "srsRAN UE not found. Docker image may not be built correctly."
        exit 1
    fi
    
    # Step 4: Wait 20 seconds for connection establishment
    log_info "Test will run for 20 seconds. Monitoring logs..."
    log_info "This allows more time for cell search and connection establishment"
    sleep 20
    
    # Step 5: Terminate processes
    log_info "Terminating processes..."
    
    # Kill tail processes first to stop log output
    if [ ! -z "$TAIL_GNB_PID" ] && kill -0 $TAIL_GNB_PID 2>/dev/null; then
        kill $TAIL_GNB_PID 2>/dev/null
    fi
    if [ ! -z "$TAIL_UE_PID" ] && kill -0 $TAIL_UE_PID 2>/dev/null; then
        kill $TAIL_UE_PID 2>/dev/null
    fi
    
    # Gracefully terminate GNodeB
    if [ ! -z "$GNODEB_PID" ] && kill -0 $GNODEB_PID 2>/dev/null; then
        kill -TERM $GNODEB_PID
        sleep 1
        # Force kill if still running
        if kill -0 $GNODEB_PID 2>/dev/null; then
            kill -KILL $GNODEB_PID
        fi
        log_info "GNodeB terminated"
    fi
    
    # Gracefully terminate UE
    if [ ! -z "$UE_PID" ] && kill -0 $UE_PID 2>/dev/null; then
        kill -TERM $UE_PID
        sleep 1
        # Force kill if still running
        if kill -0 $UE_PID 2>/dev/null; then
            kill -KILL $UE_PID
        fi
        log_info "UE terminated"
    fi
    
    # Step 6: Generate summary
    log_info "Test completed. Generating summary..."
    
    echo "=== Quick Test Summary ===" > "$LOG_DIR/summary.log"
    echo "Test Date: $(date)" >> "$LOG_DIR/summary.log"
    echo "" >> "$LOG_DIR/summary.log"
    
    # Check build status
    if [ -f "$LOG_DIR/build.log" ]; then
        if grep -q "error" "$LOG_DIR/build.log"; then
            echo "Build Status: FAILED" >> "$LOG_DIR/summary.log"
        else
            echo "Build Status: SUCCESS" >> "$LOG_DIR/summary.log"
        fi
    fi
    
    # Check GNodeB logs
    if [ "$USE_REFERENCE" = true ]; then
        if [ -f "$LOG_DIR/reference_gnb.log" ]; then
            echo "" >> "$LOG_DIR/summary.log"
            echo "Reference GNodeB Output:" >> "$LOG_DIR/summary.log"
            tail -n 20 "$LOG_DIR/reference_gnb.log" >> "$LOG_DIR/summary.log"
        fi
    else
        if [ -f "$LOG_DIR/gnodeb.log" ]; then
            echo "" >> "$LOG_DIR/summary.log"
            echo "Albor GNodeB Output:" >> "$LOG_DIR/summary.log"
            tail -n 20 "$LOG_DIR/gnodeb.log" >> "$LOG_DIR/summary.log"
        fi
    fi
    
    # Check UE logs
    if [ -f "$LOG_DIR/ue.log" ]; then
        echo "" >> "$LOG_DIR/summary.log"
        echo "UE Output:" >> "$LOG_DIR/summary.log"
        tail -n 20 "$LOG_DIR/ue.log" >> "$LOG_DIR/summary.log"
    fi
    
    # Display summary
    cat "$LOG_DIR/summary.log"
    
    log_info "All logs saved in $LOG_DIR directory:"
    if [ "$USE_REFERENCE" = false ]; then
        log_info "  - build.log: Compilation output"
        log_info "  - gnodeb.log: Albor GNodeB runtime output"
    else
        log_info "  - reference_gnb.log: Reference srsRAN gNodeB output"
    fi
    log_info "  - ue.log: srsRAN UE output"
    log_info "  - summary.log: Test summary"
    
    # Exit with appropriate code
    if grep -q "error\|failed\|Error\|Failed" "$LOG_DIR"/*.log 2>/dev/null; then
        log_error "Test completed with errors"
        exit 1
    else
        log_info "Test completed successfully"
        exit 0
    fi
    
else
    # We're NOT inside Docker container - need to check if container is running
    log_warn "Not running inside Docker container. Checking for running container..."
    
    CONTAINER_NAME="albor-gnb-dev"
    
    # Check if container is already running
    if docker ps --format '{{.Names}}' | grep -q "^${CONTAINER_NAME}$"; then
        log_info "Container '$CONTAINER_NAME' is already running. Using docker exec..."
        # Execute this script inside the running container
        # Check if we have a TTY
        if [ -t 0 ]; then
            docker exec -it "$CONTAINER_NAME" /workspace/quicktest.sh "$@"
        else
            docker exec "$CONTAINER_NAME" /workspace/quicktest.sh "$@"
        fi
        exit $?
    else
        log_warn "Container '$CONTAINER_NAME' is not running. Starting new container..."
        
        # Ensure we're in the project directory
        if [ ! -f "Dockerfile" ]; then
            log_error "Dockerfile not found. Please run from project root directory."
            exit 1
        fi
        
        # Check if Docker image exists
        IMAGE_NAME="albor-gnb-dev:latest"
        if ! docker images --format '{{.Repository}}:{{.Tag}}' | grep -q "^${IMAGE_NAME}$"; then
            log_error "Docker image '$IMAGE_NAME' not found. Please build it first with:"
            log_error "  docker build -t $IMAGE_NAME ."
            exit 1
        fi
        
        # Start container with volume mount
        log_info "Starting Docker container with volume mount..."
        docker run -it --rm \
            --name "$CONTAINER_NAME" \
            -v "$(pwd):/workspace" \
            "$IMAGE_NAME" \
            /workspace/quicktest.sh "$@"
        
        exit $?
    fi
fi