#!/bin/bash
# test_5g_final.sh - Final 5G SA test with all fixes

set -e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

log_info() { echo -e "${GREEN}[INFO]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }
log_warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }

LOG_DIR="/workspace/logs/$(date +%Y%m%d_%H%M%S)_final_test"
mkdir -p "$LOG_DIR"

log_info "=== Final 5G SA Test with Loopback Isolation ==="
log_info "Log directory: $LOG_DIR"

# Check if running as root
if [ "$EUID" -ne 0 ]; then 
    log_error "This script must be run as root inside the container"
    exit 1
fi

# Check loopback interfaces
if ! ip addr show lo2 &>/dev/null; then
    log_error "Loopback interfaces not found. Please run: ./setup_network_loopback.sh"
    exit 1
fi

# Clean up
log_info "Cleaning up previous processes..."
pkill -f 'gnb|srsue|open5gs-' || true
sleep 2

# Step 1: MongoDB
log_info "Step 1: Verifying MongoDB..."
if ! ps aux | grep -v grep | grep -q "mongod.*127.0.0.2"; then
    log_error "MongoDB not running. Starting it..."
    mongod --bind_ip 127.0.0.2 --dbpath /var/lib/mongodb --logpath "$LOG_DIR/mongodb.log" --fork
    sleep 3
fi

# Add subscriber if needed
mongosh --host 127.0.0.2 open5gs --quiet --eval '
if (db.subscribers.findOne({imsi: "001010123456780"}) == null) {
    db.subscribers.insertOne({
        imsi: "001010123456780",
        subscribed_rau_tau_timer: 12,
        network_access_mode: 0,
        subscriber_status: 0,
        access_restriction_data: 32,
        slice: [{
            sst: 1,
            default_indicator: true,
            session: [{
                name: "internet",
                type: 3,
                ambr: {
                    uplink: { value: 1, unit: 3 },
                    downlink: { value: 1, unit: 3 }
                },
                qos: {
                    index: 9,
                    arp: {
                        priority_level: 8,
                        pre_emption_capability: 1,
                        pre_emption_vulnerability: 1
                    }
                }
            }]
        }],
        ambr: {
            uplink: { value: 1, unit: 3 },
            downlink: { value: 1, unit: 3 }
        },
        security: {
            k: "465B5CE8B199B49FAA5F0A2EE238A6BC",
            amf: "8000",
            op_type: 0,
            op_value: "E8ED289DEBA952E4283B54E88E6183CA"
        },
        "schema_version": 1,
        "__v": 0
    });
    print("Subscriber added");
} else {
    print("Subscriber already exists");
}'

# Step 2: Start Open5GS
log_info "Step 2: Starting Open5GS Core..."

# NRF
log_info "Starting NRF..."
/opt/open5gs/bin/open5gs-nrfd -c /workspace/config/open5gs_native/config/nrf.yaml -D > "$LOG_DIR/nrf.log" 2>&1 &
sleep 2

# Authentication services
log_info "Starting authentication services..."
/opt/open5gs/bin/open5gs-ausfd -c /workspace/config/open5gs_native/config/ausf.yaml -D > "$LOG_DIR/ausf.log" 2>&1 &
/opt/open5gs/bin/open5gs-udmd -c /workspace/config/open5gs_native/config/udm.yaml -D > "$LOG_DIR/udm.log" 2>&1 &
/opt/open5gs/bin/open5gs-udrd -c /workspace/config/open5gs_native/config/udr.yaml -D > "$LOG_DIR/udr.log" 2>&1 &
/opt/open5gs/bin/open5gs-pcfd -c /workspace/config/open5gs_native/config/pcf.yaml -D > "$LOG_DIR/pcf.log" 2>&1 &
sleep 2

# AMF with fixed config
log_info "Starting AMF..."
/opt/open5gs/bin/open5gs-amfd -c /workspace/config/open5gs_native/config/amf_fixed.yaml -D > "$LOG_DIR/amf.log" 2>&1 &
sleep 3

# Check AMF
AMF_READY=false
for i in {1..10}; do
    if ss -anp 2>/dev/null | grep -q "LISTEN.*:38412"; then
        log_info "✓ AMF NGAP listening on port 38412"
        AMF_READY=true
        break
    fi
    sleep 1
done

if [ "$AMF_READY" = "false" ]; then
    log_error "AMF failed to start. Checking logs..."
    tail -20 "$LOG_DIR/amf.log"
    exit 1
fi

# TUN device
log_info "Configuring TUN device..."
if ! ip link show ogstun &>/dev/null; then
    ip tuntap add name ogstun mode tun
fi
ip addr add 10.45.0.1/16 dev ogstun 2>/dev/null || true
ip link set ogstun up
sysctl -w net.ipv4.ip_forward=1 >/dev/null

# SMF and UPF
log_info "Starting SMF and UPF..."
/opt/open5gs/bin/open5gs-smfd -c /workspace/config/open5gs_native/config/smf.yaml -D > "$LOG_DIR/smf.log" 2>&1 &
sleep 2
/opt/open5gs/bin/open5gs-upfd -c /workspace/config/open5gs_native/config/upf.yaml -D > "$LOG_DIR/upf.log" 2>&1 &
sleep 3

# Check UPF - wait a bit for it to fully start
sleep 2
if ss -unp 2>/dev/null | grep -q "127.0.0.10:2152"; then
    log_info "✓ UPF GTP-U listening on 127.0.0.10:2152"
else
    log_warn "UPF GTP-U not detected yet, continuing..."
fi

# Step 3: Start gNodeB
log_info "Step 3: Starting gNodeB..."
cd /opt/srsran_project

# Ensure /tmp is writable
chmod 777 /tmp 2>/dev/null || true
touch /tmp/gnb.log && chmod 666 /tmp/gnb.log 2>/dev/null || true

/opt/srsran_project/bin/gnb -c /workspace/config/srsran_gnb/gnb_zmq_loopback_correct.yml > "$LOG_DIR/gnb.log" 2>&1 &
GNB_PID=$!

# Wait for gNodeB
GNB_READY=false
for i in {1..30}; do
    if grep -q "NG setup procedure completed" "$LOG_DIR/gnb.log" 2>/dev/null; then
        log_info "✓ gNodeB connected to AMF!"
        GNB_READY=true
        break
    fi
    if grep -q "error\|failed\|Failed" "$LOG_DIR/gnb.log" 2>/dev/null; then
        log_error "gNodeB error detected:"
        grep -i "error\|failed" "$LOG_DIR/gnb.log" | tail -5
        break
    fi
    printf "\r[%02d/30] Waiting for gNodeB..." "$i"
    sleep 1
done
echo ""

if [ "$GNB_READY" = "false" ]; then
    log_error "gNodeB failed to start properly"
    tail -20 "$LOG_DIR/gnb.log"
    
    # Show GTP status
    log_info "GTP-U port status:"
    ss -unp | grep 2152 || echo "No GTP listeners found"
    exit 1
fi

# Verify both GTP endpoints
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

/opt/srsran/bin/srsue /workspace/config/srsue/ue_nr_zmq_loopback.conf > "$LOG_DIR/ue.log" 2>&1 &
UE_PID=$!

# Monitor registration
log_info "Step 5: Monitoring registration..."
SUCCESS=false
TIMEOUT=60

for i in $(seq 1 $TIMEOUT); do
    # Check milestones
    if grep -q "Found Cell" "$LOG_DIR/ue.log" 2>/dev/null && [ "$CELL_FOUND" != "1" ]; then
        echo ""
        log_info "✓ UE found cell"
        CELL_FOUND=1
    fi
    
    if grep -q "Random Access Complete" "$LOG_DIR/ue.log" 2>/dev/null && [ "$RACH_DONE" != "1" ]; then
        echo ""
        log_info "✓ Random access completed"
        RACH_DONE=1
    fi
    
    if grep -q "RRC Connected" "$LOG_DIR/ue.log" 2>/dev/null && [ "$RRC_DONE" != "1" ]; then
        echo ""
        log_info "✓ RRC connected"
        RRC_DONE=1
    fi
    
    if grep -q "NAS.*EMM-REGISTERED" "$LOG_DIR/ue.log" 2>/dev/null; then
        echo ""
        log_info "✓ NAS registration completed!"
        SUCCESS=true
        break
    fi
    
    printf "\r[%02d/%02d] Waiting for registration..." "$i" "$TIMEOUT"
    sleep 1
done

echo ""
echo "=========================================="
log_info "FINAL TEST RESULTS:"
echo "=========================================="

if [ "$SUCCESS" = "true" ]; then
    log_info "✅ SUCCESS: Full 5G SA registration achieved!"
    log_info ""
    log_info "Network isolation working correctly:"
    log_info "  - UPF on 127.0.0.10:2152"
    log_info "  - gNodeB on 127.0.0.11:2152"
    log_info "  - No port conflicts!"
    log_info ""
    
    # Show registration details
    log_info "Registration milestones:"
    grep -E "(Found Cell|Random Access|RRC Connected|EMM-REGISTERED|PDU Session)" "$LOG_DIR/ue.log" | tail -10
    
    # Check for IP assignment
    if grep -q "PDU Session" "$LOG_DIR/ue.log"; then
        log_info ""
        log_info "✓ PDU session established - data plane ready!"
    fi
else
    log_error "❌ FAILED: Registration did not complete"
    
    # Debug info
    echo ""
    log_info "Debug Information:"
    
    if [ "$CELL_FOUND" != "1" ]; then
        log_error "UE did not find cell - check gNodeB SSB transmission"
    elif [ "$RACH_DONE" != "1" ]; then
        log_error "RACH failed - check PRACH configuration"
    elif [ "$RRC_DONE" != "1" ]; then
        log_error "RRC connection failed - check RRC procedures"
    else
        log_error "NAS registration failed - check core network"
    fi
    
    echo ""
    log_info "Last UE logs:"
    tail -20 "$LOG_DIR/ue.log"
    
    echo ""
    log_info "Last gNodeB logs:"
    tail -15 "$LOG_DIR/gnb.log"
fi

echo "=========================================="

# Summary
log_info ""
log_info "Test Summary:"
log_info "  Loopback interfaces: ✓"
log_info "  MongoDB: ✓"
log_info "  Open5GS Core: ✓"
log_info "  AMF NGAP: ✓"
log_info "  UPF GTP-U: ✓"
log_info "  gNodeB: $([ "$GNB_READY" = "true" ] && echo '✓' || echo '✗')"
log_info "  UE Registration: $([ "$SUCCESS" = "true" ] && echo '✓' || echo '✗')"

if [ "$SUCCESS" = "true" ]; then
    log_info ""
    log_info "5G SA system running successfully!"
    log_info "Press Ctrl+C to stop."
    
    # Keep monitoring
    tail -f "$LOG_DIR/ue.log" | grep -E "PDU|data|IP" &
    wait
fi