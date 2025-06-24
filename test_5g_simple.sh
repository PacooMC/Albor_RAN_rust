#!/bin/bash
# test_5g_simple.sh - Simplified 5G SA test with loopback interfaces

set -e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

log_info() { echo -e "${GREEN}[INFO]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }
log_warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }

LOG_DIR="/workspace/logs/$(date +%Y%m%d_%H%M%S)_simple_test"
mkdir -p "$LOG_DIR"

log_info "=== Simplified 5G SA Test ==="

# Check loopback interfaces
if ! ip addr show lo2 &>/dev/null; then
    log_error "Loopback interfaces not found. Please run as root: ./setup_network_loopback.sh"
    exit 1
fi

# Kill existing processes
pkill -f 'gnb|srsue|open5gs-' || true
sleep 2

# Step 1: Check MongoDB
log_info "Step 1: Checking MongoDB..."
if ! ps aux | grep -v grep | grep -q "mongod.*127.0.0.2"; then
    log_info "Starting MongoDB..."
    mongod --bind_ip 127.0.0.2 --dbpath /var/lib/mongodb --logpath "$LOG_DIR/mongodb.log" --fork || true
    sleep 3
else
    log_info "MongoDB already running"
fi

# Step 2: Start minimal Open5GS
log_info "Step 2: Starting minimal Open5GS..."

# Start NRF
log_info "Starting NRF on 127.0.0.3:7777..."
/opt/open5gs/bin/open5gs-nrfd -c /workspace/config/open5gs_native/config/nrf.yaml > "$LOG_DIR/nrf.log" 2>&1 &
NRF_PID=$!
sleep 3

# Check NRF
if netstat -tuln | grep -q "127.0.0.3:7777"; then
    log_info "✓ NRF listening on 127.0.0.3:7777"
else
    log_error "NRF failed to start"
    tail -20 "$LOG_DIR/nrf.log"
    exit 1
fi

# Start AMF
log_info "Starting AMF on 127.0.0.4..."
/opt/open5gs/bin/open5gs-amfd -c /workspace/config/open5gs_native/config/amf_simple.yaml > "$LOG_DIR/amf.log" 2>&1 &
AMF_PID=$!
sleep 3

# Check AMF SCTP
if ss -anp | grep -q "LISTEN.*:38412"; then
    log_info "✓ AMF NGAP listening on port 38412"
else
    log_error "AMF NGAP not listening"
    tail -20 "$LOG_DIR/amf.log"
fi

# Configure TUN for UPF
log_info "Configuring TUN device..."
ip tuntap add name ogstun mode tun || true
ip addr add 10.45.0.1/16 dev ogstun || true
ip link set ogstun up
sysctl -w net.ipv4.ip_forward=1
iptables -t nat -A POSTROUTING -s 10.45.0.0/16 ! -o ogstun -j MASQUERADE || true

# Start minimal services for registration
log_info "Starting authentication services..."
/opt/open5gs/bin/open5gs-ausfd -c /workspace/config/open5gs_native/config/ausf.yaml > "$LOG_DIR/ausf.log" 2>&1 &
/opt/open5gs/bin/open5gs-udmd -c /workspace/config/open5gs_native/config/udm.yaml > "$LOG_DIR/udm.log" 2>&1 &
/opt/open5gs/bin/open5gs-udrd -c /workspace/config/open5gs_native/config/udr.yaml > "$LOG_DIR/udr.log" 2>&1 &
sleep 2

# Start SMF and UPF
log_info "Starting SMF and UPF..."
/opt/open5gs/bin/open5gs-smfd -c /workspace/config/open5gs_native/config/smf.yaml > "$LOG_DIR/smf.log" 2>&1 &
/opt/open5gs/bin/open5gs-upfd -c /workspace/config/open5gs_native/config/upf.yaml > "$LOG_DIR/upf.log" 2>&1 &
sleep 3

# Check UPF GTP
if ss -unp | grep -q "127.0.0.10:2152"; then
    log_info "✓ UPF GTP-U listening on 127.0.0.10:2152"
else
    log_error "UPF GTP-U not listening"
fi

# Step 3: Start gNodeB
log_info "Step 3: Starting gNodeB..."
cd /opt/srsran_project
/opt/srsran_project/bin/gnb -c /workspace/config/srsran_gnb/gnb_zmq_loopback.yml > "$LOG_DIR/gnb.log" 2>&1 &
GNB_PID=$!

# Wait for gNodeB
for i in {1..20}; do
    if grep -q "NG setup procedure completed" "$LOG_DIR/gnb.log" 2>/dev/null; then
        log_info "✓ gNodeB connected to AMF!"
        break
    fi
    if grep -q "Failed to bind" "$LOG_DIR/gnb.log" 2>/dev/null; then
        log_error "gNodeB failed to bind GTP-U"
        tail -20 "$LOG_DIR/gnb.log"
        
        # Check both GTP endpoints
        log_info "GTP-U status:"
        ss -unp | grep 2152
        exit 1
    fi
    sleep 1
done

# Check GTP-U bindings
log_info ""
log_info "GTP-U Endpoints:"
ss -unp | grep 2152 | while read line; do
    echo "  $line"
done

# Step 4: Start UE
log_info ""
log_info "Step 4: Starting UE..."
cd /opt/srsran
export LD_LIBRARY_PATH=/opt/srsran/lib:$LD_LIBRARY_PATH
/opt/srsran/bin/srsue /workspace/config/srsue/ue_nr_zmq.conf > "$LOG_DIR/ue.log" 2>&1 &
UE_PID=$!

# Monitor
log_info "Monitoring registration..."
SUCCESS=false

for i in {1..60}; do
    if grep -q "RRC Connected" "$LOG_DIR/ue.log" 2>/dev/null; then
        if ! [ "$RRC_SHOWN" = "1" ]; then
            log_info "✓ RRC Connected"
            RRC_SHOWN=1
        fi
    fi
    
    if grep -q "NAS.*EMM-REGISTERED" "$LOG_DIR/ue.log" 2>/dev/null; then
        log_info "✓ UE Registered!"
        SUCCESS=true
        break
    fi
    
    printf "\r[%02d/60] Waiting for registration..." "$i"
    sleep 1
done

echo ""
if [ "$SUCCESS" = "true" ]; then
    log_info "✅ SUCCESS: 5G SA connection established!"
    log_info ""
    log_info "Key milestones:"
    grep -E "(Found Cell|Random Access|RRC Connected|EMM-REGISTERED)" "$LOG_DIR/ue.log" | tail -10
else
    log_error "❌ Registration failed"
    log_info "UE log:"
    tail -20 "$LOG_DIR/ue.log"
    log_info ""
    log_info "gNodeB log:" 
    tail -20 "$LOG_DIR/gnb.log"
fi

# Keep running
if [ "$SUCCESS" = "true" ]; then
    log_info ""
    log_info "System running. Press Ctrl+C to stop."
    wait
else
    # Cleanup on failure
    kill $GNB_PID $UE_PID 2>/dev/null || true
fi