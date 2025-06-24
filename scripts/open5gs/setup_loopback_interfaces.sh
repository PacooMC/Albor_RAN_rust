#!/bin/bash
# Setup loopback interfaces for Open5GS components to avoid port conflicts
# Creates interfaces 127.0.0.2 through 127.0.0.12 for isolated component deployment

set -e

echo "=== Setting up loopback interfaces for Open5GS ==="

# Function to check if an interface exists
interface_exists() {
    local addr=$1
    ip addr show | grep -q "inet $addr/8" && return 0 || return 1
}

# Function to add loopback interface
add_loopback() {
    local addr=$1
    local name=$2
    
    if interface_exists "$addr"; then
        echo "  ✓ Interface $addr already exists ($name)"
    else
        echo "  + Adding interface $addr for $name"
        sudo ip addr add "$addr/8" dev lo 2>/dev/null || {
            echo "    Warning: Failed to add $addr (may require privileges)"
            return 1
        }
    fi
}

# Check if we have proper permissions
if ! sudo -n true 2>/dev/null; then
    echo "This script requires sudo privileges to add network interfaces."
    echo "Please run with: sudo $0"
    exit 1
fi

# Network assignment plan
echo "Network assignment plan:"
echo "  - MongoDB:     127.0.0.2:27017"
echo "  - NRF:         127.0.0.3:7777"
echo "  - AMF:         127.0.0.4:38412 (SCTP), 127.0.0.4:7777 (HTTP)"
echo "  - SMF:         127.0.0.5:7777"
echo "  - PCF:         127.0.0.6:7777"
echo "  - UDR:         127.0.0.7:7777"
echo "  - UDM:         127.0.0.8:7777"
echo "  - AUSF:        127.0.0.9:7777"
echo "  - UPF:         127.0.0.10:2152 (GTP-U), 127.0.0.10:8805 (PFCP)"
echo "  - NSSF:        127.0.0.11:7777"
echo "  - BSF:         127.0.0.12:7777"
echo ""

# Add loopback interfaces
echo "Adding loopback interfaces..."
add_loopback "127.0.0.2" "MongoDB"
add_loopback "127.0.0.3" "NRF"
add_loopback "127.0.0.4" "AMF"
add_loopback "127.0.0.5" "SMF"
add_loopback "127.0.0.6" "PCF"
add_loopback "127.0.0.7" "UDR"
add_loopback "127.0.0.8" "UDM"
add_loopback "127.0.0.9" "AUSF"
add_loopback "127.0.0.10" "UPF"
add_loopback "127.0.0.11" "NSSF"
add_loopback "127.0.0.12" "BSF"

# Verify interfaces
echo ""
echo "Verifying interfaces..."
MISSING=0
for i in {2..12}; do
    if interface_exists "127.0.0.$i"; then
        echo "  ✓ 127.0.0.$i is active"
    else
        echo "  ✗ 127.0.0.$i is missing"
        MISSING=$((MISSING + 1))
    fi
done

if [ $MISSING -gt 0 ]; then
    echo ""
    echo "Warning: $MISSING interfaces could not be created."
    echo "You may need to run this script with proper privileges."
    exit 1
fi

# Enable IP forwarding for UPF functionality
echo ""
echo "Enabling IP forwarding..."
sudo sysctl -w net.ipv4.ip_forward=1 >/dev/null 2>&1 || echo "  Warning: Could not enable IP forwarding"
sudo sysctl -w net.ipv6.conf.all.forwarding=1 >/dev/null 2>&1 || echo "  Warning: Could not enable IPv6 forwarding"

# Check SCTP module
echo ""
echo "Checking SCTP support..."
if lsmod | grep -q sctp; then
    echo "  ✓ SCTP module is loaded"
else
    echo "  ! SCTP module not loaded, attempting to load..."
    sudo modprobe sctp 2>/dev/null || {
        echo "  ✗ Failed to load SCTP module"
        echo "    AMF may not be able to bind to SCTP port 38412"
        echo "    In Docker, you may need to run with --privileged or --cap-add=NET_ADMIN"
    }
fi

# Create marker file to indicate setup is complete
sudo touch /var/run/open5gs_loopback_setup 2>/dev/null || true

echo ""
echo "=== Loopback interface setup complete ==="
echo "All interfaces are ready for Open5GS deployment."
echo ""
echo "Next steps:"
echo "1. Run ./update_open5gs_configs.sh to update configurations"
echo "2. Run ./start_open5gs_core.sh to start Open5GS"
echo ""