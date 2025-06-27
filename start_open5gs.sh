#\!/bin/bash

# Start Open5GS components for Albor 5G testing
# This script starts all necessary Open5GS components in the correct order

echo "==== Starting Open5GS components ===="

# Kill any existing Open5GS processes
echo "Killing any existing Open5GS processes..."
pkill -f open5gs || true
sleep 1

# Check if MongoDB is running, start if not
if \! pgrep -x "mongod" > /dev/null; then
    echo "Starting MongoDB..."
    mongod --fork --logpath /var/log/mongodb.log --dbpath /var/lib/mongodb
    sleep 2
else
    echo "MongoDB is already running"
fi

# Function to start a component and check if it started
start_component() {
    local component=$1
    echo "Starting $component..."
    /open5gs/install/bin/open5gs-${component}d -D
    sleep 1
    if pgrep -f "open5gs-${component}d" > /dev/null; then
        echo "$component started successfully"
    else
        echo "ERROR: Failed to start $component"
        exit 1
    fi
}

# Start components in order
# NRF must start first
start_component "nrf"
sleep 2

# Start other components
start_component "udr"
start_component "udm"
start_component "ausf"
start_component "bsf"
start_component "pcf"
start_component "smf"
start_component "amf"
start_component "upf"

echo "All Open5GS components started successfully"
echo ""

# Add test subscriber if not already exists
echo "Adding test subscriber..."
/open5gs/install/bin/open5gs-dbctl add_ue_with_ki 001010000000001 465B5CE8B199B49FAA5F0A2EE238A6BC E8ED289DEBA952E4283B54E88E6183CA

echo ""
echo "Open5GS is ready\!"
echo "AMF listening on: 127.0.0.1:38412 (NGAP/SCTP)"
echo "NRF listening on: 127.0.0.1:7777 (HTTP)"
echo "Test subscriber IMSI: 001010000000001"
echo ""

# Keep script running and monitor processes
echo "Monitoring Open5GS processes (press Ctrl+C to stop all)..."
trap 'echo "Stopping Open5GS..."; pkill -f open5gs; exit' INT
while true; do
    sleep 5
    # Check if all components are still running
    for comp in nrf udr udm ausf bsf pcf smf amf upf; do
        if \! pgrep -f "open5gs-${comp}d" > /dev/null; then
            echo "WARNING: $comp has stopped\!"
        fi
    done
done
