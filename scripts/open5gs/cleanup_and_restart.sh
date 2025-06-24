#!/bin/bash
# cleanup_and_restart.sh - Clean up and restart Open5GS

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

log_info() { echo -e "${GREEN}[INFO]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }
log_warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }

log_info "=== Cleaning up and restarting Open5GS ==="

# Step 1: Kill all Open5GS processes
log_info "Killing all Open5GS processes..."
for comp in nrf amf smf upf ausf udm udr pcf nssf bsf; do
    pkill -9 open5gs-${comp}d 2>/dev/null || true
done

# Kill any remaining open5gs processes
pkill -9 -f open5gs 2>/dev/null || true

# Step 2: Clean up zombie processes
log_info "Cleaning up zombie processes..."
ps aux | grep defunct | awk '{print $2}' | xargs -r kill -9 2>/dev/null || true

# Step 3: Clean up MongoDB
log_info "Restarting MongoDB..."
pkill -9 mongod 2>/dev/null || true
sleep 2

# Start MongoDB
mongod --dbpath /var/lib/mongodb --logpath /var/log/open5gs/mongodb.log --bind_ip 127.0.0.2 --fork

# Wait for MongoDB
for i in {1..10}; do
    if netstat -tuln 2>/dev/null | grep -q "127.0.0.2:27017"; then
        log_info "✓ MongoDB started on 127.0.0.2:27017"
        break
    fi
    sleep 1
done

# Step 4: Start Open5GS components in order
log_info "Starting Open5GS components..."

# Start NRF first
log_info "Starting NRF..."
open5gs-nrfd -c /etc/open5gs/nrf.yaml -d &
sleep 3

# Start other components
for comp in amf smf upf ausf udm udr pcf nssf bsf; do
    log_info "Starting $comp..."
    open5gs-${comp}d -c /etc/open5gs/${comp}.yaml -d &
    sleep 1
done

# Wait for AMF to be ready
log_info "Waiting for AMF to be ready..."
for i in {1..20}; do
    if netstat -tuln 2>/dev/null | grep -q "127.0.0.4:38412"; then
        log_info "✓ AMF is listening on 127.0.0.4:38412"
        break
    fi
    if [ $i -eq 20 ]; then
        log_error "AMF failed to start"
        exit 1
    fi
    sleep 1
done

# Step 5: Add test subscriber
log_info "Adding test subscriber..."
cd /opt/open5gs

cat > add_subscriber.js << 'EOF'
db = db.getSiblingDB('open5gs');

// Remove existing subscriber
db.subscribers.deleteOne({ "imsi": "001010000000001" });

// Add new subscriber
db.subscribers.insertOne({
    "imsi": "001010000000001",
    "msisdn": ["0000000001"],
    "imeisv": "3534900698733190",
    "mme_host": [],
    "mme_realm": [],
    "purge_flag": [],
    "security": {
        "k": "00112233445566778899AABBCCDDEEFF",
        "op": null,
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
            "qos": {
                "index": 9,
                "arp": {
                    "priority_level": 8,
                    "pre_emption_capability": 1,
                    "pre_emption_vulnerability": 1
                }
            },
            "ambr": {
                "downlink": { "value": 1, "unit": 3 },
                "uplink": { "value": 1, "unit": 3 }
            },
            "ue": {
                "addr": null,
                "addr6": null
            },
            "smf": {
                "addr": null,
                "addr6": null
            },
            "pcc_rule": []
        }]
    }],
    "access_restriction_data": 32,
    "operator_determined_barring": 0,
    "subscribed_rau_tau_timer": 12,
    "network_access_mode": 0,
    "ue_ambr": {
        "downlink": { "value": 1, "unit": 3 },
        "uplink": { "value": 1, "unit": 3 }
    },
    "__v": 0
});

print("Subscriber added successfully");
EOF

mongo 127.0.0.2:27017 add_subscriber.js

log_info "✓ Open5GS cleanup and restart complete!"

# Show status
log_info "Component status:"
ps aux | grep open5gs | grep -v grep | grep -v defunct | awk '{print "  - " $11 " (PID: " $2 ")"}'

log_info "Network status:"
netstat -tuln | grep -E "(127.0.0.[2-9]|127.0.0.1[0-2])" | grep LISTEN | awk '{print "  - " $4}'