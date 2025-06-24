#!/bin/bash
# Open5GS Status Script

echo "=== Open5GS 5G Core Status ==="
echo ""

# Check MongoDB
echo "MongoDB Status:"
if pgrep -x mongod > /dev/null; then
    echo "  ✓ MongoDB is running (PID: $(pgrep -x mongod))"
else
    echo "  ✗ MongoDB is not running"
fi

# Check TUN device
echo ""
echo "TUN Device Status:"
if ip link show ogstun > /dev/null 2>&1; then
    echo "  ✓ ogstun is configured"
    ip addr show ogstun | grep -E "inet|inet6" | sed 's/^/    /'
else
    echo "  ✗ ogstun is not configured"
fi

# Check Open5GS components
echo ""
echo "Open5GS Components:"
for component in nrf amf smf upf ausf udm udr pcf nssf bsf; do
    if pgrep -f "open5gs-${component}d" > /dev/null; then
        pid=$(pgrep -f "open5gs-${component}d")
        echo "  ✓ ${component}: running (PID: $pid)"
    else
        echo "  ✗ ${component}: not running"
    fi
done

# Check AMF SCTP port
echo ""
echo "Network Status:"
if netstat -tulpn 2>/dev/null | grep -q ":38412.*LISTEN"; then
    echo "  ✓ AMF NGAP port (38412/sctp) is listening"
else
    echo "  ✗ AMF NGAP port (38412/sctp) is not listening"
fi

# Check NRF HTTP port
if netstat -tulpn 2>/dev/null | grep -q ":7777.*LISTEN"; then
    echo "  ✓ NRF HTTP port (7777/tcp) is listening"
else
    echo "  ✗ NRF HTTP port (7777/tcp) is not listening"
fi

echo ""