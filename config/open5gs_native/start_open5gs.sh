#!/bin/bash
# start_open5gs.sh - Start Open5GS with multiple loopback interfaces

set -e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

log_info() { echo -e "${GREEN}[INFO]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }
log_warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }

CONFIG_DIR="/workspace/config/open5gs_native/config"
LOG_DIR="/workspace/logs/open5gs"

# Create log directory
mkdir -p "$LOG_DIR"

# Function to stop all Open5GS processes
stop_open5gs() {
    log_info "Stopping Open5GS services..."
    pkill -f open5gs- || true
    sleep 2
}

# Cleanup on exit
trap stop_open5gs EXIT

log_info "=== Starting Open5GS Core Network ==="

# Check if loopback interfaces exist
if ! ip addr show lo2 &>/dev/null; then
    log_error "Loopback interfaces not found. Please run setup_network_loopback.sh first"
    exit 1
fi

# Stop any existing Open5GS processes
stop_open5gs

# Start MongoDB on lo2
log_info "Starting MongoDB..."
mongod --bind_ip 127.0.0.2 --logpath "$LOG_DIR/mongodb.log" --fork

# Wait for MongoDB
sleep 3

# Add test subscriber
log_info "Adding test subscriber..."
mongosh --host 127.0.0.2 open5gs --eval '
db.subscribers.deleteMany({});
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
})'

# Start NRF first (service registry)
log_info "Starting NRF..."
/opt/open5gs/bin/open5gs-nrfd -c "$CONFIG_DIR/nrf.yaml" -D > "$LOG_DIR/nrf.log" 2>&1 &
sleep 2

# Start authentication services
log_info "Starting authentication services..."
/opt/open5gs/bin/open5gs-ausfd -c "$CONFIG_DIR/ausf.yaml" -D > "$LOG_DIR/ausf.log" 2>&1 &
/opt/open5gs/bin/open5gs-udmd -c "$CONFIG_DIR/udm.yaml" -D > "$LOG_DIR/udm.log" 2>&1 &
/opt/open5gs/bin/open5gs-udrd -c "$CONFIG_DIR/udr.yaml" -D > "$LOG_DIR/udr.log" 2>&1 &
/opt/open5gs/bin/open5gs-pcfd -c "$CONFIG_DIR/pcf.yaml" -D > "$LOG_DIR/pcf.log" 2>&1 &
sleep 2

# Start AMF
log_info "Starting AMF..."
/opt/open5gs/bin/open5gs-amfd -c "$CONFIG_DIR/amf.yaml" -D > "$LOG_DIR/amf.log" 2>&1 &
sleep 2

# Start SMF
log_info "Starting SMF..."
/opt/open5gs/bin/open5gs-smfd -c "$CONFIG_DIR/smf.yaml" -D > "$LOG_DIR/smf.log" 2>&1 &
sleep 2

# Configure TUN device for UPF
log_info "Configuring TUN device..."
ip tuntap add name ogstun mode tun || true
ip addr add 10.45.0.1/16 dev ogstun || true
ip link set ogstun up
# Enable IP forwarding
sysctl -w net.ipv4.ip_forward=1
# Add NAT rule for UE traffic
iptables -t nat -A POSTROUTING -s 10.45.0.0/16 ! -o ogstun -j MASQUERADE || true

# Start UPF
log_info "Starting UPF..."
/opt/open5gs/bin/open5gs-upfd -c "$CONFIG_DIR/upf.yaml" -D > "$LOG_DIR/upf.log" 2>&1 &
sleep 2

# Check if all services are running
log_info "Checking Open5GS services..."
SERVICES=(nrfd ausfd udmd udrd pcfd amfd smfd upfd)
ALL_RUNNING=true

for svc in "${SERVICES[@]}"; do
    if pgrep -f "open5gs-$svc" > /dev/null; then
        log_info "✓ open5gs-$svc is running"
    else
        log_error "✗ open5gs-$svc is NOT running"
        ALL_RUNNING=false
    fi
done

# Check AMF NGAP port
if ss -anp | grep -q ":38412.*LISTEN"; then
    log_info "✓ AMF NGAP listening on port 38412"
else
    log_error "✗ AMF NGAP not listening on port 38412"
fi

# Check UPF GTP port
if ss -anp | grep -q "127.0.0.10:2152"; then
    log_info "✓ UPF GTP-U listening on 127.0.0.10:2152"
else
    log_error "✗ UPF GTP-U not listening"
fi

if [ "$ALL_RUNNING" = "true" ]; then
    log_info "✅ All Open5GS services started successfully!"
    log_info ""
    log_info "Network Configuration:"
    log_info "  MongoDB:   127.0.0.2:27017"
    log_info "  NRF:       127.0.0.3:7777"
    log_info "  AMF:       127.0.0.4:38412 (NGAP), 127.0.0.4:7777 (SBI)"
    log_info "  SMF:       127.0.0.5:7777"
    log_info "  UPF:       127.0.0.10:2152 (GTP-U)"
    log_info "  UE subnet: 10.45.0.0/16"
    log_info ""
    log_info "gNodeB should connect to AMF at: 127.0.0.4:38412"
    log_info "gNodeB should bind GTP-U to: 127.0.0.11:2152"
else
    log_error "Some services failed to start. Check logs in $LOG_DIR"
    exit 1
fi

# Keep script running
log_info "Open5GS is running. Press Ctrl+C to stop."
wait