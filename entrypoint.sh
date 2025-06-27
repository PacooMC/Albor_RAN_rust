#!/bin/bash
# Entrypoint script for Albor Docker container with Open5GS

echo "=== Starting Albor/Open5GS Container ==="

# Start MongoDB
echo "Starting MongoDB service..."
mkdir -p /data/db /var/log/mongodb
mongod --bind_ip 127.0.0.1 --logpath /var/log/mongodb/mongod.log --dbpath /data/db --fork

# Wait for MongoDB
echo "Waiting for MongoDB to be ready..."
for i in {1..30}; do
    if nc -z 127.0.0.1 27017; then
        echo "MongoDB is ready!"
        break
    fi
    echo "Waiting for MongoDB... ($i/30)"
    sleep 1
done

# Setup network interface for Open5GS
echo "Setting up network interfaces..."
if ! grep "ogstun" /proc/net/dev > /dev/null; then
    ip tuntap add name ogstun mode tun
fi
ip addr del 10.45.0.1/16 dev ogstun 2> /dev/null
ip addr add 10.45.0.1/16 dev ogstun
ip addr del 2001:db8:cafe::1/48 dev ogstun 2> /dev/null
ip addr add 2001:db8:cafe::1/48 dev ogstun
ip link set ogstun up

# Enable IP forwarding
sysctl -w net.ipv4.ip_forward=1

# Start Open5GS components if requested
if [ "${START_OPEN5GS}" = "true" ]; then
    echo "Starting Open5GS services..."
    
    # Create log directory
    mkdir -p /var/log/open5gs
    
    # Start Open5GS services in order
    echo "Starting NRF..."
    /open5gs/install/bin/open5gs-nrfd -D &
    sleep 1
    
    echo "Starting SCP..."
    /open5gs/install/bin/open5gs-scpd -D &
    sleep 1
    
    echo "Starting AMF..."
    /open5gs/install/bin/open5gs-amfd -D &
    sleep 1
    
    # Skip SMF and UPF as they conflict with gNodeB port 2152
    # echo "Starting SMF..."
    # /open5gs/install/bin/open5gs-smfd -D &
    
    # echo "Starting UPF..."
    # /open5gs/install/bin/open5gs-upfd -D &
    
    echo "Starting AUSF..."
    /open5gs/install/bin/open5gs-ausfd -D &
    
    echo "Starting UDM..."
    /open5gs/install/bin/open5gs-udmd -D &
    
    echo "Starting UDR..."
    /open5gs/install/bin/open5gs-udrd -D &
    
    echo "Starting PCF..."
    /open5gs/install/bin/open5gs-pcfd -D &
    
    echo "Starting NSSF..."
    /open5gs/install/bin/open5gs-nssfd -D &
    
    echo "Starting BSF..."
    /open5gs/install/bin/open5gs-bsfd -D &
    
    sleep 2
    echo "Open5GS services started!"
fi

echo "Container is ready. Keeping it running..."
tail -f /dev/null