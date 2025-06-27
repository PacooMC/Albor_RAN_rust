#!/bin/bash
# Fix and test srsRAN with Open5GS in single container
# This script prepares everything from the host and restarts the container

set +e

# Colors
GREEN='\033[0;32m'
RED='\033[0;31m'
NC='\033[0m'

log_info() { echo -e "${GREEN}[INFO]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }

CONTAINER_NAME="albor-gnb-dev"
LOG_DIR="logs/$(date +%Y%m%d_%H%M%S)_srsran_baseline"
mkdir -p "$LOG_DIR"

log_info "=== Establishing srsRAN + Open5GS Baseline ==="

# Step 1: Create startup script from host
log_info "Step 1: Creating startup script for Open5GS + srsRAN..."
cat > docker-open5gs-allinone/startup_baseline.sh << 'EOF'
#!/bin/bash
set -e

# Kill any existing processes
pkill -9 open5gs || true
pkill -9 mongod || true
pkill -9 gnb || true
pkill -9 srsue || true
sleep 2

# Fix Open5GS configurations to use correct addresses
echo "Fixing Open5GS configurations..."
sed -i 's/127.0.0.10/127.0.0.1/g' /open5gs/install/etc/open5gs/*.yaml
sed -i 's/::1/127.0.0.1/g' /open5gs/install/etc/open5gs/*.yaml
# Fix the IPv6 address issue in SMF/UPF configs
sed -i 's/2001:db8:cafe127.0.0.1/2001:db8:cafe::1/g' /open5gs/install/etc/open5gs/*.yaml

# Start MongoDB
echo "Starting MongoDB..."
mkdir -p /data/db
mongod --bind_ip 127.0.0.1 --logpath /tmp/mongodb.log --fork
sleep 3

# Start Open5GS components
echo "Starting Open5GS components..."
cd /open5gs/install/bin

# Start NRF first
./open5gs-nrfd -D
sleep 2

# Start SCP
./open5gs-scpd -D
sleep 1

# Start database-related components
./open5gs-udrd -D
./open5gs-udmd -D
./open5gs-ausfd -D
./open5gs-bsfd -D
./open5gs-pcfd -D
./open5gs-nssfd -D
sleep 2

# Start SMF and UPF
./open5gs-smfd -D
./open5gs-upfd -D
sleep 2

# Start AMF last
./open5gs-amfd -D
sleep 3

echo "Open5GS started. Checking AMF SCTP listener..."
ss -lnS | grep 38412

# Add test subscriber
echo "Adding test subscriber..."
mongosh 127.0.0.1:27017/open5gs --quiet --eval '
db.subscribers.deleteOne({ "imsi": "001010000000001" });
db.subscribers.insertOne({
    "imsi": "001010000000001",
    "msisdn": ["0000000001"],
    "imeisv": "353490069873319",
    "security": {
        "k": "465B5CE8B199B49FAA5F0A2EE238A6BC",
        "opc": "E8ED289DEBA952E4283B54E88E6183CA",
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

echo "Subscriber added. Ready for gNodeB connection."
EOF

chmod +x docker-open5gs-allinone/startup_baseline.sh

# Step 2: Stop and restart container
log_info "Step 2: Restarting container with new configuration..."
docker compose down
docker compose up -d

# Wait for container to be healthy
log_info "Waiting for container to start..."
sleep 10

# Step 3: Execute startup script
log_info "Step 3: Starting Open5GS and MongoDB..."
docker compose exec -T $CONTAINER_NAME /workspace/docker-open5gs-allinone/startup_baseline.sh

# Step 4: Verify AMF is listening
log_info "Step 4: Verifying AMF SCTP listener..."
AMF_LISTENING=$(docker compose exec -T $CONTAINER_NAME bash -c "ss -lnS | grep 38412")
if [ -z "$AMF_LISTENING" ]; then
    log_error "AMF is not listening on SCTP port 38412!"
    docker compose exec -T $CONTAINER_NAME bash -c "tail -20 /open5gs/install/var/log/open5gs/amf.log"
    exit 1
fi
log_info "✓ AMF listening on: $AMF_LISTENING"

# Step 5: Start srsRAN gNodeB
log_info "Step 5: Starting srsRAN gNodeB..."
docker compose exec -d $CONTAINER_NAME bash -c "
    cd /workspace
    /usr/local/bin/gnb -c /workspace/config/srsran_gnb/gnb_zmq.yml > /workspace/$LOG_DIR/gnb_stdout.log 2>&1
"

# Wait for AMF connection
log_info "Waiting for gNodeB to connect to AMF..."
sleep 5

# Check if connected
GNB_CONNECTED=$(docker compose exec -T $CONTAINER_NAME bash -c "grep -E '(NG setup procedure completed|Connected to AMF)' /workspace/$LOG_DIR/gnb_stdout.log 2>/dev/null || grep -E '(NG setup procedure completed|Connected to AMF)' /workspace/$LOG_DIR/gnb.log 2>/dev/null")
if [ -n "$GNB_CONNECTED" ]; then
    log_info "✅ gNodeB connected to AMF!"
else
    log_error "❌ gNodeB failed to connect to AMF"
    log_info "gNodeB log:"
    docker compose exec -T $CONTAINER_NAME tail -30 "/workspace/$LOG_DIR/gnb_stdout.log" 2>/dev/null || echo "No log"
fi

# Step 6: Start srsUE
log_info "Step 6: Starting srsUE..."
docker compose exec -d $CONTAINER_NAME bash -c "
    export LD_LIBRARY_PATH=/usr/local/lib:\$LD_LIBRARY_PATH
    /usr/local/bin/srsue /workspace/config/srsue/ue_zmq.conf > /workspace/$LOG_DIR/ue_stdout.log 2>&1
"

# Step 7: Monitor for 15 seconds
log_info "Step 7: Monitoring for 15 seconds..."
for i in {1..15}; do
    printf "\r[%02d/15] Monitoring..." "$i"
    
    # Check for cell found
    CELL_FOUND=$(docker compose exec -T $CONTAINER_NAME grep "Found Cell" "/workspace/$LOG_DIR/ue_stdout.log" 2>/dev/null)
    if [ -n "$CELL_FOUND" ] && [ "$CELL_MSG" != "shown" ]; then
        echo ""
        log_info "✅ UE found cell!"
        CELL_MSG="shown"
    fi
    
    # Check for RRC
    RRC_CONNECTED=$(docker compose exec -T $CONTAINER_NAME grep "RRC Connected" "/workspace/$LOG_DIR/ue_stdout.log" 2>/dev/null)
    if [ -n "$RRC_CONNECTED" ] && [ "$RRC_MSG" != "shown" ]; then
        echo ""
        log_info "✅ RRC connected!"
        RRC_MSG="shown"
    fi
    
    # Check for registration
    REGISTERED=$(docker compose exec -T $CONTAINER_NAME grep -E "(NAS-5G.*Registration complete|5GMM-REGISTERED)" "/workspace/$LOG_DIR/ue_stdout.log" 2>/dev/null)
    if [ -n "$REGISTERED" ]; then
        echo ""
        log_info "✅ UE registered to 5G network!"
        break
    fi
    
    sleep 1
done

echo ""
log_info "Stopping all processes..."
docker compose exec -T $CONTAINER_NAME bash -c "pkill -9 gnb || true; pkill -9 srsue || true"

# Show results
echo ""
log_info "=== RESULTS ==="
log_info "Checking baseline success criteria:"

# Check all 6 criteria
CRITERIA_MET=0

# 1. Open5GS running with SCTP functional
if [ -n "$AMF_LISTENING" ]; then
    log_info "✅ 1. Open5GS running with SCTP functional"
    ((CRITERIA_MET++))
else
    log_error "❌ 1. Open5GS NOT running with SCTP"
fi

# 2. srsRAN gNodeB: "NG setup procedure completed"
if [ -n "$GNB_CONNECTED" ]; then
    log_info "✅ 2. srsRAN gNodeB: NG setup procedure completed"
    ((CRITERIA_MET++))
else
    log_error "❌ 2. srsRAN gNodeB: NG setup NOT completed"
fi

# 3. srsRAN gNodeB: Connected to AMF
if [ -n "$GNB_CONNECTED" ]; then
    log_info "✅ 3. srsRAN gNodeB: Connected to AMF"
    ((CRITERIA_MET++))
else
    log_error "❌ 3. srsRAN gNodeB: NOT connected to AMF"
fi

# 4. srsUE: "Found Cell"
if [ -n "$CELL_FOUND" ]; then
    log_info "✅ 4. srsUE: Found Cell"
    ((CRITERIA_MET++))
else
    log_error "❌ 4. srsUE: Cell NOT found"
fi

# 5. srsUE: "RRC Connected"
if [ -n "$RRC_CONNECTED" ]; then
    log_info "✅ 5. srsUE: RRC Connected"
    ((CRITERIA_MET++))
else
    log_error "❌ 5. srsUE: RRC NOT connected"
fi

# 6. srsUE: "NAS-5G Registration complete"
if [ -n "$REGISTERED" ]; then
    log_info "✅ 6. srsUE: NAS-5G Registration complete"
    ((CRITERIA_MET++))
else
    log_error "❌ 6. srsUE: NAS-5G Registration NOT complete"
fi

echo ""
log_info "Success criteria met: $CRITERIA_MET/6"

if [ $CRITERIA_MET -eq 6 ]; then
    log_info "🎉 BASELINE ESTABLISHED! All criteria met!"
    log_info "You can now proceed with Albor testing."
else
    log_error "❌ BASELINE NOT ESTABLISHED! Only $CRITERIA_MET/6 criteria met."
    log_error "DO NOT proceed with Albor until all 6 criteria are met."
fi

echo ""
log_info "Full logs saved to: $LOG_DIR"