#!/bin/bash
# test_srsran.sh - Complete 5G SA test with srsRAN gNodeB + UE + Open5GS
# Uses docker-open5gs for reliable core network deployment

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
LOG_DIR="/tmp/logs/$(date +%Y%m%d_%H%M%S)_srsran_full"
mkdir -p "$LOG_DIR"

log_info "=== srsRAN Complete 5G SA Test ==="
log_info "Testing: Open5GS + srsRAN gNodeB + srsRAN UE"
log_info "Log directory: $LOG_DIR"

# Cleanup function
cleanup() {
    log_info "Cleaning up..."
    # Kill UE and gNodeB
    if [ "$IN_DOCKER" = "1" ]; then
        pkill -f 'gnb|srsue' || true
    else
        docker exec albor-gnb-dev bash -c "pkill -f 'gnb|srsue' || true" 2>/dev/null || true
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
log_info "Step 1: Starting Open5GS Core Network..."

# Change to Open5GS directory
cd config/open5gs

# Check if Open5GS images are built
if ! docker images | grep -q "amf.*v2.7.2"; then
    log_error "Open5GS images not found. Please run ./build_open5gs_images.sh first"
    exit 1
fi

# Stop any existing Open5GS containers
docker compose -f docker-compose-docker-open5gs.yml down -v 2>/dev/null || true

# Start Open5GS using the docker-open5gs configuration
log_info "Starting Open5GS with docker-open5gs configuration..."
docker compose -f docker-compose-docker-open5gs.yml up -d

log_info "Waiting for Open5GS to fully initialize..."

# Wait a bit for containers to start
sleep 5

# Check if AMF has initialized
for i in {1..25}; do
    if docker logs open5gs_amf 2>&1 | grep -q "AMF initialize.*done"; then
        log_info "✓ Open5GS AMF is ready and NGAP server listening on port 38412"
        break
    fi
    
    if [ $i -eq 25 ]; then
        log_error "Open5GS AMF failed to initialize properly"
        docker logs open5gs_amf 2>&1 | tail -50
        exit 1
    fi
    
    printf "\r[%02d/25] Waiting for AMF initialization..." "$i"
    sleep 1
done
echo ""

log_info "Waiting additional 10s for all components to stabilize..."
sleep 10

# Verify Open5GS components are running
if docker ps | grep -q open5gs_amf && docker ps | grep -q open5gs_smf && docker ps | grep -q open5gs_upf; then
    log_info "✓ All Open5GS containers are running"
    # Check AMF logs
    if docker logs open5gs_amf 2>&1 | grep -q "ngap"; then
        log_info "✓ AMF NGAP interface ready"
    fi
    # Check MongoDB subscriber
    if docker exec open5gs_db mongosh open5gs --eval "db.subscribers.findOne({imsi: '001010123456780'})" | grep -q "001010123456780"; then
        log_info "✓ Subscriber found in database"
    else
        log_warn "Subscriber not found, adding manually..."
        docker exec open5gs_db mongosh open5gs < mongo-init.js
    fi
else
    log_error "✗ Open5GS containers failed to start"
    docker compose -f docker-compose-docker-open5gs.yml ps
    exit 1
fi

cd ../..

# Kill existing processes
log_info "Stopping any existing processes..."
cleanup
sleep 2

# Step 2: Configure and start gNodeB
log_info "Step 2: Starting srsRAN gNodeB..."

# AMF IP is fixed in our docker-compose configuration
AMF_IP="10.53.1.2"
log_info "AMF IP address: $AMF_IP"

# Create gNodeB config with correct TAC and parameters
cat > /tmp/gnb_config.yml << EOF
cu_cp:
  amf:
    addr: $AMF_IP
    port: 38412
    bind_addr: 10.53.1.50
    supported_tracking_areas:
      - tac: 7
        plmn_list:
          - plmn: "00101"
            tai_slice_support_list:
              - sst: 1
  inactivity_timer: 7200

ru_sdr:
  device_driver: zmq
  device_args: tx_port=tcp://127.0.0.1:2000,rx_port=tcp://127.0.0.1:2001,base_srate=23.04e6
  srate: 23.04
  tx_gain: 75
  rx_gain: 75

cell_cfg:
  dl_arfcn: 368500
  band: 3
  channel_bandwidth_MHz: 20
  common_scs: 15
  plmn: "00101"
  tac: 7
  pci: 1
  
  pdcch:
    common:
      ss0_index: 0
      coreset0_index: 13
    dedicated:
      ss2_type: common
      dci_format_0_1_and_1_1: false
  prach:
    prach_config_index: 1
  pdsch:
    mcs_table: qam64
  pusch:
    mcs_table: qam64

log:
  filename: $LOG_DIR/gnb.log
  all_level: info
  phy_level: info
  mac_level: info
  rlc_level: info
  pdcp_level: info
  rrc_level: info
  ngap_level: info

pcap:
  mac_enable: false
  mac_filename: $LOG_DIR/gnb_mac.pcap
  ngap_enable: false
  ngap_filename: $LOG_DIR/gnb_ngap.pcap
EOF

# Copy config to container
docker cp /tmp/gnb_config.yml albor-gnb-dev:/tmp/gnb_config.yml

# Ensure albor-gnb-dev container is running
if ! docker ps | grep -q albor-gnb-dev; then
    log_error "albor-gnb-dev container is not running. Please start it first."
    exit 1
fi

# Connect albor-gnb-dev to Open5GS network with specific IP
log_info "Connecting albor-gnb-dev container to Open5GS network..."
# Disconnect first if already connected
docker network disconnect open5gs_net albor-gnb-dev 2>/dev/null || true
# Connect with specific IP address
docker network connect --ip 10.53.1.50 open5gs_net albor-gnb-dev

# Create log directory in container
docker exec albor-gnb-dev mkdir -p "$LOG_DIR"

# Start gNodeB
docker exec albor-gnb-dev bash -c "
cd /opt/srsran_project
mkdir -p $LOG_DIR
/opt/srsran_project/bin/gnb -c /tmp/gnb_config.yml > $LOG_DIR/gnb.log 2>&1 &
echo \$!
" > /tmp/gnb_pid.txt

GNB_PID=$(cat /tmp/gnb_pid.txt)
log_info "gNodeB started (PID: $GNB_PID)"

# Wait for AMF connection
log_info "Waiting for gNodeB to connect to AMF..."
for i in {1..30}; do
    if docker exec albor-gnb-dev grep -q "NG setup procedure completed" "$LOG_DIR/gnb.log" 2>/dev/null; then
        log_info "✓ gNodeB connected to AMF successfully!"
        break
    fi
    if [ $i -eq 30 ]; then
        log_error "✗ gNodeB failed to connect to AMF"
        docker exec albor-gnb-dev tail -20 "$LOG_DIR/gnb.log"
        exit 1
    fi
    sleep 1
done

# Step 3: Start srsUE
log_info "Step 3: Starting srsUE..."

docker exec albor-gnb-dev bash -c "
cd /opt/srsran
export LD_LIBRARY_PATH=/opt/srsran/lib:\$LD_LIBRARY_PATH
mkdir -p $LOG_DIR
/opt/srsran/bin/srsue /workspace/config/srsue/ue_nr_zmq.conf > $LOG_DIR/ue.log 2>&1 &
echo \$!
" > /tmp/ue_pid.txt

UE_PID=$(cat /tmp/ue_pid.txt)
log_info "srsUE started (PID: $UE_PID)"

# Step 4: Monitor registration
log_info "Step 4: Monitoring 5G registration..."

TIMEOUT=60
SUCCESS=false

for i in $(seq 1 $TIMEOUT); do
    printf "\r[%02d/%02d] Checking registration status..." "$i" "$TIMEOUT"
    
    # Check various stages
    if docker exec albor-gnb-dev grep -q "Found Cell" "$LOG_DIR/ue.log" 2>/dev/null; then
        if ! [ "$CELL_FOUND" = "1" ]; then
            echo ""
            log_info "✓ UE found cell"
            CELL_FOUND=1
        fi
    fi
    
    if docker exec albor-gnb-dev grep -q "Random Access Complete" "$LOG_DIR/ue.log" 2>/dev/null; then
        if ! [ "$RACH_COMPLETE" = "1" ]; then
            echo ""
            log_info "✓ Random access completed"
            RACH_COMPLETE=1
        fi
    fi
    
    if docker exec albor-gnb-dev grep -q "RRC Connected" "$LOG_DIR/ue.log" 2>/dev/null; then
        if ! [ "$RRC_CONNECTED" = "1" ]; then
            echo ""
            log_info "✓ RRC connected"
            RRC_CONNECTED=1
        fi
    fi
    
    if docker exec albor-gnb-dev grep -q "NAS" "$LOG_DIR/ue.log" 2>/dev/null | grep -q "EMM-REGISTERED"; then
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
    
    # Show key milestones
    docker exec albor-gnb-dev grep -E "(Found Cell|Random Access|RRC Connected|EMM-REGISTERED|PDU Session)" "$LOG_DIR/ue.log" | tail -20
    
    # Check AMF logs
    echo ""
    log_info "AMF status:"
    docker logs open5gs_amf 2>&1 | grep -E "(Registered|PDU|Session|InitialUEMessage)" | tail -5
else
    log_error "❌ FAILED: UE did not register"
    
    # Debug info
    echo ""
    log_info "gNodeB log tail:"
    docker exec albor-gnb-dev tail -30 "$LOG_DIR/gnb.log"
    
    echo ""
    log_info "UE log tail:"
    docker exec albor-gnb-dev tail -30 "$LOG_DIR/ue.log"
    
    echo ""
    log_info "AMF log tail:"
    docker logs open5gs_amf 2>&1 | tail -20
    echo ""
    log_info "SMF log tail:"
    docker logs open5gs_smf 2>&1 | tail -10
fi

echo "=========================================="

# Keep running if successful
if [ "$SUCCESS" = "true" ]; then
    log_info "System is running. Press Ctrl+C to stop."
    wait
fi