#!/bin/bash
# setup_loopback_robust.sh - Robust loopback interface setup

set -e

# Check if we have NET_ADMIN capability
if ! capsh --print 2>/dev/null | grep -q "cap_net_admin"; then
    echo "ERROR: Need NET_ADMIN capability to create network interfaces!"
    echo "Please run container with --cap-add=NET_ADMIN"
    exit 1
fi

# Function to safely add loopback alias
add_loopback_alias() {
    local num=$1
    local ip="127.0.0.$num"
    
    # Check if already exists
    if ip addr show lo | grep -q "$ip/8"; then
        echo "✓ Loopback alias $ip already exists"
    else
        echo "Creating loopback alias $ip..."
        ip addr add $ip/8 dev lo || {
            echo "Failed to add $ip - retrying with sudo"
            sudo ip addr add $ip/8 dev lo || {
                echo "ERROR: Cannot add loopback alias $ip"
                return 1
            }
        }
        echo "✓ Added loopback alias $ip"
    fi
}

# Add all required loopback aliases
for i in {2..11}; do
    add_loopback_alias $i
done

# Verify all interfaces
echo ""
echo "Verifying loopback interfaces:"
ip addr show lo | grep "127.0.0." | while read line; do
    echo "  $line"
done

echo ""
echo "✅ Loopback interface setup complete"
