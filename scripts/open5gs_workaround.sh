#!/bin/bash
# Open5GS Workaround Script for Docker Permission Limitations
# This script works around:
# - No --privileged flag
# - SCTP module can't load
# - Permission issues with /var/log/open5gs/

set -e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

log_info() { echo -e "${GREEN}[INFO]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }
log_warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }

# Create alternative log directory in workspace
LOG_BASE="/workspace/logs/open5gs_$(date +%Y%m%d_%H%M%S)"
mkdir -p "$LOG_BASE"

log_info "=== Open5GS Workaround Setup ==="
log_info "Working around Docker permission limitations"
log_info "Log directory: $LOG_BASE"

# Step 1: Create modified Open5GS configurations
log_info "Step 1: Creating modified Open5GS configurations..."

CONFIG_DIR="/workspace/config/open5gs_workaround"
mkdir -p "$CONFIG_DIR"

# Copy original configs
cp -r /opt/open5gs/etc/open5gs/* "$CONFIG_DIR/"

# Modify configurations to work without SCTP and with alternative paths
log_info "Modifying configurations for workaround..."

# AMF: Try to use TCP fallback if available, or at least configure proper paths
cat > "$CONFIG_DIR/amf.yaml" << EOF
logger:
  file: $LOG_BASE/amf.log
  level: info

global:
  max_integrity_protected_data_rate: 
    uplink: 64kbps
    downlink: 64kbps

amf:
  sbi:
    server:
      - address: 127.0.0.5
        port: 7777
    client:
      scp:
        - uri: http://127.0.0.200:7777
  ngap:
    server:
      - address: 127.0.0.5
        port: 38412
  guami:
    - plmn_id:
        mcc: 001
        mnc: 01
      amf_id:
        region: 2
        set: 1
  tai:
    - plmn_id:
        mcc: 001
        mnc: 01
      tac: 7
  plmn_support:
    - plmn_id:
        mcc: 001
        mnc: 01
      s_nssai:
        - sst: 1
  security:
    integrity_order: [ NIA2, NIA1, NIA0 ]
    ciphering_order: [ NEA0, NEA1, NEA2 ]
  network_name:
    full: Open5GS
    short: Open5GS
  network_feature_support_5gs:
    enable: true
    ims_voice_over_ps_session: 0
    emc: 0
    emf: 0
    iwk_n26: 0
    mpsi: 0
    emcn3: 0
    mcsi: 0
  amf_name: open5gs-amf0
  time:
    t3502:
      value: 720
    t3512:
      value: 540
    t3513:
      minimum: 2
      value: 2
EOF

# Create configs for other components with proper log paths
for component in nrf scp smf upf bsf udm udr ausf nssf pcf; do
    if [ -f "/opt/open5gs/etc/open5gs/${component}.yaml" ]; then
        # Replace log file path
        sed "s|file:.*|file: $LOG_BASE/${component}.log|" \
            "/opt/open5gs/etc/open5gs/${component}.yaml" > "$CONFIG_DIR/${component}.yaml"
    fi
done

# Step 2: Setup MongoDB with workspace directory
log_info "Step 2: Setting up MongoDB..."

# Kill any existing MongoDB
pkill -9 mongod 2>/dev/null || true
sleep 2

# Clean up locks
rm -f /workspace/mongodb-data/mongod.lock
rm -f /workspace/mongodb-data/WiredTiger.lock

# Start MongoDB with workspace data directory
mongod --dbpath /workspace/mongodb-data \
       --logpath "$LOG_BASE/mongodb.log" \
       --bind_ip 127.0.0.2 \
       --fork \
       --quiet \
       --wiredTigerCacheSizeGB 0.5 || {
    log_error "Failed to start MongoDB"
    exit 1
}

# Wait for MongoDB
for i in {1..30}; do
    if netstat -tuln 2>/dev/null | grep -q "127.0.0.2:27017"; then
        log_info "✓ MongoDB started successfully"
        break
    fi
    printf "\r[%02d/30] Waiting for MongoDB..." "$i"
    sleep 1
done
echo ""

# Step 3: Add test subscriber
log_info "Step 3: Adding test subscriber..."

# Check if mongo client is available
if command -v mongo >/dev/null 2>&1; then
    mongo 127.0.0.2:27017/open5gs --quiet --eval '
    db.subscribers.deleteOne({ "imsi": "001010000000001" });
    db.subscribers.insertOne({
        "imsi": "001010000000001",
        "msisdn": ["0000000001"],
        "imeisv": "353490069873310",
        "security": {
            "k": "00112233445566778899AABBCCDDEEFF",
            "opc": "63BFA50EE6523365FF14C1F45F88737D",
            "amf": "8000"
        },
        "ambr": {
            "downlink": { "value": 1, "unit": 3 },
            "uplink": { "value": 1, "unit": 3 }
        },
        "slice": [{
            "sst": 1,
            "default_indicator": true,
            "session": [{
                "name": "internet",
                "type": 3,
                "qos": { "index": 9 },
                "ambr": {
                    "downlink": { "value": 1, "unit": 3 },
                    "uplink": { "value": 1, "unit": 3 }
                }
            }]
        }],
        "access_restriction_data": 32,
        "subscribed_rau_tau_timer": 12,
        "network_access_mode": 0
    });'
    log_info "✓ Test subscriber added"
else
    log_warn "mongo client not available - subscriber must be added manually"
    log_warn "You can add it later using the Open5GS WebUI or API"
fi

# Step 4: Start Open5GS components with workaround configs
log_info "Step 4: Starting Open5GS components..."

# Kill any existing Open5GS processes
for comp in nrf scp amf smf upf bsf udm udr ausf nssf pcf; do
    pkill -9 -f "open5gs-${comp}d" 2>/dev/null || true
done
sleep 2

# Function to start Open5GS component
start_component() {
    local component=$1
    local binary="/opt/open5gs/bin/open5gs-${component}d"
    local config="$CONFIG_DIR/${component}.yaml"
    
    if [ -x "$binary" ] && [ -f "$config" ]; then
        log_info "Starting open5gs-${component}d..."
        
        # Set LD_LIBRARY_PATH for Open5GS libraries
        export LD_LIBRARY_PATH=/opt/open5gs/lib:$LD_LIBRARY_PATH
        
        # Start with config file
        $binary -c "$config" -D > "$LOG_BASE/${component}_stdout.log" 2>&1 &
        
        # Store PID for monitoring
        echo $! > "$LOG_BASE/${component}.pid"
        
        sleep 1
        
        # Check if process started
        if ps -p $! > /dev/null; then
            log_info "✓ open5gs-${component}d started (PID: $!)"
            return 0
        else
            log_error "✗ open5gs-${component}d failed to start"
            cat "$LOG_BASE/${component}_stdout.log" | tail -20
            return 1
        fi
    fi
    return 0
}

# Start components in order
for component in nrf scp; do
    start_component $component || log_warn "Failed to start $component"
done

# Give NRF and SCP time to initialize
sleep 3

# Try to start AMF without SCTP
log_info "Attempting to start AMF (SCTP may fail)..."

# First try: Direct execution with environment setup
export LD_LIBRARY_PATH=/opt/open5gs/lib:$LD_LIBRARY_PATH

# Check if we can at least bind to the port
if ! netstat -tuln 2>/dev/null | grep -q ":38412"; then
    log_info "Port 38412 is available, attempting AMF start..."
    
    # Try starting AMF - it may fail on SCTP socket creation
    /opt/open5gs/bin/open5gs-amfd -c "$CONFIG_DIR/amf.yaml" -D > "$LOG_BASE/amf_stdout.log" 2>&1 &
    AMF_PID=$!
    echo $AMF_PID > "$LOG_BASE/amf.pid"
    
    sleep 2
    
    if ps -p $AMF_PID > /dev/null 2>&1; then
        log_info "✓ AMF process started (PID: $AMF_PID)"
    else
        log_warn "AMF failed to start - checking logs..."
        tail -20 "$LOG_BASE/amf_stdout.log"
        
        # Try alternative: Mock AMF endpoint for testing
        log_info "Creating mock AMF endpoint for testing..."
        
        # Create a simple TCP listener on port 38412 as fallback
        python3 -c "
import socket
import threading
import time

def handle_connection(conn, addr):
    print(f'Connection from {addr}')
    # Just accept connections and log them
    while True:
        try:
            data = conn.recv(1024)
            if not data:
                break
            print(f'Received {len(data)} bytes from {addr}')
        except:
            break
    conn.close()

def amf_mock():
    sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    sock.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
    sock.bind(('127.0.0.5', 38412))
    sock.listen(5)
    print('Mock AMF listening on 127.0.0.5:38412 (TCP)')
    
    while True:
        conn, addr = sock.accept()
        thread = threading.Thread(target=handle_connection, args=(conn, addr))
        thread.daemon = True
        thread.start()

try:
    amf_mock()
except KeyboardInterrupt:
    pass
" > "$LOG_BASE/mock_amf_tcp.log" 2>&1 &
        
        echo $! > "$LOG_BASE/mock_amf.pid"
        log_info "Mock AMF TCP endpoint started"
    fi
else
    log_warn "Port 38412 already in use"
fi

# Start remaining components
for component in smf upf bsf udm udr ausf nssf pcf; do
    start_component $component || log_warn "Failed to start $component"
done

# Step 5: Check status
log_info "Step 5: Checking Open5GS status..."

echo ""
log_info "Running processes:"
ps aux | grep -E "open5gs|mongod" | grep -v grep || echo "No Open5GS processes found"

echo ""
log_info "Listening ports:"
netstat -tuln 2>/dev/null | grep -E "127\.0\.0\." || echo "No Open5GS ports found"

echo ""
log_info "Component logs are in: $LOG_BASE"
log_info "MongoDB data is in: /workspace/mongodb-data"

echo ""
log_warn "IMPORTANT NOTES:"
log_warn "1. AMF may not work properly without SCTP module"
log_warn "2. Using alternative log directory: $LOG_BASE"
log_warn "3. This is a workaround - full functionality not guaranteed"

# Save workaround info
cat > "$LOG_BASE/workaround_info.txt" << EOF
Open5GS Workaround Information
==============================
Date: $(date)
Log Directory: $LOG_BASE
Config Directory: $CONFIG_DIR

Issues Worked Around:
1. No --privileged flag in Docker
2. SCTP module cannot be loaded
3. /var/log/open5gs/ permission issues

Solutions Applied:
1. Using workspace directory for logs: $LOG_BASE
2. Modified configurations to use alternative paths
3. Attempted TCP fallback for AMF (may not fully work)
4. MongoDB using workspace data directory

Limitations:
- AMF NGAP interface requires SCTP, may not function properly
- Some Open5GS features may be limited without proper permissions
- This is a temporary workaround for testing purposes

To use with srsRAN:
- srsRAN gNodeB may need to be configured for TCP instead of SCTP
- Connection to AMF may fail due to SCTP requirement
- Consider using mock endpoints for testing if needed
EOF

log_info "Workaround setup complete!"
log_info "Check $LOG_BASE/workaround_info.txt for details"