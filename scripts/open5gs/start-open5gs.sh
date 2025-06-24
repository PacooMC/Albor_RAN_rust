#!/bin/bash
# Open5GS Startup Script for localhost deployment
# This script starts all Open5GS components for 5G SA operation

set -e

echo "=== Starting Open5GS 5G Core ==="

# Create necessary directories
sudo mkdir -p /var/log/open5gs
sudo mkdir -p /var/run

# Set up TUN device
echo "Setting up TUN device..."
if ! ip link show ogstun > /dev/null 2>&1; then
    sudo ip tuntap add name ogstun mode tun
fi
sudo ip addr add 10.45.0.1/16 dev ogstun 2>/dev/null || true
sudo ip addr add 2001:db8:cafe::1/48 dev ogstun 2>/dev/null || true
sudo ip link set ogstun up

# Start MongoDB if not running
echo "Starting MongoDB..."
if ! pgrep -x mongod > /dev/null; then
    sudo systemctl start mongod 2>/dev/null || sudo mongod --fork --logpath /var/log/mongodb.log --dbpath /var/lib/mongodb
fi

# Wait for MongoDB to be ready
echo "Waiting for MongoDB..."
for i in {1..10}; do
    if mongosh --eval "db.adminCommand('ping')" > /dev/null 2>&1; then
        echo "MongoDB is ready"
        break
    fi
    sleep 1
done

# Function to start a component
start_component() {
    local component=$1
    local config_file="/opt/open5gs/etc/open5gs/${component}.yaml"
    
    if [ -f "$config_file" ]; then
        echo "Starting ${component}..."
        /opt/open5gs/bin/open5gs-${component}d -c "$config_file" -d &
        echo "${component} started with PID $!"
    else
        echo "Warning: Configuration file for ${component} not found at $config_file"
    fi
}

# Start components in order
# NRF must start first
start_component nrf
sleep 2

# Start other NF components
start_component amf
start_component smf
start_component upf
start_component ausf
start_component udm
start_component udr
start_component pcf
start_component nssf
start_component bsf

echo ""
echo "=== Open5GS 5G Core Started ==="
echo "AMF listening on: 127.0.0.1:38412 (NGAP/SCTP)"
echo "MongoDB running on: 127.0.0.1:27017"
echo "TUN device: ogstun (10.45.0.1/16)"
echo ""
echo "To stop Open5GS, run: ./scripts/open5gs/stop-open5gs.sh"
echo "To check status, run: ./scripts/open5gs/status-open5gs.sh"
echo ""