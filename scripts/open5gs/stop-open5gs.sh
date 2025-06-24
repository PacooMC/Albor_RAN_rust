#!/bin/bash
# Open5GS Stop Script

echo "=== Stopping Open5GS 5G Core ==="

# Stop all Open5GS components
for component in nrf amf smf upf ausf udm udr pcf nssf bsf; do
    if pgrep -f "open5gs-${component}d" > /dev/null; then
        echo "Stopping ${component}..."
        sudo pkill -f "open5gs-${component}d"
    fi
done

# Optional: Stop MongoDB (commented out by default)
# echo "Stopping MongoDB..."
# sudo systemctl stop mongod 2>/dev/null || sudo pkill mongod

# Optional: Remove TUN device (commented out by default)
# echo "Removing TUN device..."
# sudo ip link delete ogstun 2>/dev/null || true

echo "=== Open5GS 5G Core Stopped ==="