#!/bin/bash
# Enhanced Open5GS startup script with multi-loopback support
# Starts all Open5GS components on unique IP addresses to avoid conflicts

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
LOG_DIR="/var/log/open5gs"
PID_DIR="/var/run/open5gs"

echo "=== Starting Open5GS 5G Core (Multi-Loopback Edition) ==="
echo "Timestamp: $(date)"

# Function to check prerequisites
check_prerequisites() {
    local issues=0
    
    echo "Checking prerequisites..."
    
    # Check if Open5GS is installed
    if [ ! -d "/opt/open5gs/bin" ]; then
        echo "  ✗ Open5GS not found at /opt/open5gs/bin"
        issues=$((issues + 1))
    else
        echo "  ✓ Open5GS installation found"
    fi
    
    # Check if loopback interfaces are set up
    if [ ! -f "/var/run/open5gs_loopback_setup" ]; then
        echo "  ! Loopback interfaces may not be configured"
        echo "    Running setup script..."
        if [ -x "${SCRIPT_DIR}/setup_loopback_interfaces.sh" ]; then
            "${SCRIPT_DIR}/setup_loopback_interfaces.sh"
        else
            echo "  ✗ setup_loopback_interfaces.sh not found or not executable"
            issues=$((issues + 1))
        fi
    else
        echo "  ✓ Loopback interfaces already configured"
    fi
    
    # Verify key interfaces exist
    for ip in 127.0.0.{2..12}; do
        if ! ip addr show | grep -q "inet $ip/8"; then
            echo "  ✗ Interface $ip is missing"
            issues=$((issues + 1))
        fi
    done
    
    # Check SCTP support
    if ! lsmod | grep -q sctp; then
        echo "  ! SCTP module not loaded"
        echo "    Attempting to load SCTP module..."
        if ! sudo modprobe sctp 2>/dev/null; then
            echo "  ✗ Failed to load SCTP module - AMF may not work properly"
            echo "    In Docker, run with: --privileged or --cap-add=NET_ADMIN,SYS_ADMIN"
            # Don't count as critical issue - might work without module in some environments
        fi
    else
        echo "  ✓ SCTP support available"
    fi
    
    return $issues
}

# Function to set up directories and permissions
setup_directories() {
    echo "Setting up directories..."
    sudo mkdir -p "$LOG_DIR" "$PID_DIR"
    sudo chmod 755 "$LOG_DIR" "$PID_DIR"
}

# Function to start MongoDB
start_mongodb() {
    echo "Starting MongoDB..."
    
    # Check if MongoDB is already running
    if pgrep -x mongod > /dev/null; then
        echo "  ✓ MongoDB is already running"
        
        # Verify it's listening on the correct address
        if netstat -tuln 2>/dev/null | grep -q "127.0.0.2:27017"; then
            echo "  ✓ MongoDB listening on 127.0.0.2:27017"
        else
            echo "  ! MongoDB not listening on expected address 127.0.0.2:27017"
            echo "    Restarting MongoDB with correct configuration..."
            sudo systemctl stop mongod 2>/dev/null || sudo pkill mongod
            sleep 2
            start_mongodb_daemon
        fi
    else
        start_mongodb_daemon
    fi
}

# Function to start MongoDB daemon
start_mongodb_daemon() {
    # Try systemctl first
    if command -v systemctl >/dev/null 2>&1; then
        if sudo systemctl start mongod 2>/dev/null; then
            echo "  ✓ MongoDB started via systemctl"
        else
            # Fallback to direct execution
            start_mongodb_direct
        fi
    else
        start_mongodb_direct
    fi
    
    # Wait for MongoDB to be ready
    echo "  Waiting for MongoDB to be ready..."
    for i in {1..30}; do
        if mongosh --host 127.0.0.2 --eval "db.adminCommand('ping')" >/dev/null 2>&1; then
            echo "  ✓ MongoDB is ready"
            return 0
        fi
        sleep 1
    done
    
    echo "  ✗ MongoDB failed to start properly"
    return 1
}

# Function to start MongoDB directly
start_mongodb_direct() {
    local mongo_cmd="mongod --bind_ip 127.0.0.2,127.0.0.1 --port 27017"
    
    # Check if we have a data directory
    if [ -d "/var/lib/mongodb" ]; then
        mongo_cmd="$mongo_cmd --dbpath /var/lib/mongodb"
    elif [ -d "/data/db" ]; then
        mongo_cmd="$mongo_cmd --dbpath /data/db"
    else
        # Create a data directory
        sudo mkdir -p /var/lib/mongodb
        sudo chown -R $(whoami) /var/lib/mongodb 2>/dev/null || true
        mongo_cmd="$mongo_cmd --dbpath /var/lib/mongodb"
    fi
    
    # Start MongoDB in background
    sudo $mongo_cmd --fork --logpath ${LOG_DIR}/mongodb.log
}

# Function to set up TUN device for UPF
setup_tun_device() {
    echo "Setting up TUN device for UPF..."
    
    # Create TUN device if it doesn't exist
    if ! ip link show ogstun >/dev/null 2>&1; then
        echo "  Creating ogstun device..."
        sudo ip tuntap add name ogstun mode tun
    fi
    
    # Add IP addresses
    sudo ip addr add 10.45.0.1/16 dev ogstun 2>/dev/null || echo "  ! IPv4 address already assigned"
    sudo ip addr add 2001:db8:cafe::1/48 dev ogstun 2>/dev/null || echo "  ! IPv6 address already assigned"
    
    # Bring up the interface
    sudo ip link set ogstun up
    echo "  ✓ TUN device ogstun configured"
    
    # Enable IP forwarding
    sudo sysctl -w net.ipv4.ip_forward=1 >/dev/null 2>&1
    sudo sysctl -w net.ipv6.conf.all.forwarding=1 >/dev/null 2>&1
}

# Function to start a component with proper error handling
start_component() {
    local component=$1
    local ip_addr=$2
    local special_ports=$3
    local config_file="/opt/open5gs/etc/open5gs/${component}.yaml"
    local binary="/opt/open5gs/bin/open5gs-${component}d"
    local pid_file="${PID_DIR}/${component}.pid"
    local log_file="${LOG_DIR}/${component}.log"
    
    echo "Starting $component on $ip_addr..."
    
    # Check if already running
    if [ -f "$pid_file" ] && kill -0 $(cat "$pid_file") 2>/dev/null; then
        echo "  ✓ $component is already running (PID: $(cat $pid_file))"
        return 0
    fi
    
    # Check if binary exists
    if [ ! -x "$binary" ]; then
        echo "  ✗ Binary not found: $binary"
        return 1
    fi
    
    # Check if config exists
    if [ ! -f "$config_file" ]; then
        echo "  ✗ Config not found: $config_file"
        return 1
    fi
    
    # Start the component
    sudo -E $binary -c "$config_file" -d > "$log_file" 2>&1 &
    local pid=$!
    echo $pid | sudo tee "$pid_file" > /dev/null
    
    # Wait a moment for startup
    sleep 2
    
    # Verify it's running
    if kill -0 $pid 2>/dev/null; then
        echo "  ✓ $component started (PID: $pid)"
        
        # Check if it's listening on expected ports
        if [ -n "$special_ports" ]; then
            echo "  Checking ports: $special_ports"
            for port in $special_ports; do
                if netstat -tuln 2>/dev/null | grep -q "$ip_addr:$port"; then
                    echo "    ✓ Listening on $ip_addr:$port"
                else
                    echo "    ! Not listening on $ip_addr:$port (may take a moment)"
                fi
            done
        fi
        
        return 0
    else
        echo "  ✗ $component failed to start"
        echo "  Check log: $log_file"
        tail -5 "$log_file" 2>/dev/null
        return 1
    fi
}

# Function to wait for NRF to be ready
wait_for_nrf() {
    echo "Waiting for NRF to be fully ready..."
    for i in {1..30}; do
        if curl -s http://127.0.0.3:7777/nnrf-nfm/v1/nf-instances >/dev/null 2>&1; then
            echo "  ✓ NRF is ready"
            return 0
        fi
        sleep 1
    done
    echo "  ✗ NRF not responding after 30 seconds"
    return 1
}

# Main execution
main() {
    # Check prerequisites
    if ! check_prerequisites; then
        echo "Prerequisites check failed. Please fix the issues and try again."
        exit 1
    fi
    
    # Setup directories
    setup_directories
    
    # Start MongoDB
    if ! start_mongodb; then
        echo "Failed to start MongoDB. Aborting."
        exit 1
    fi
    
    # Setup TUN device
    setup_tun_device
    
    echo ""
    echo "Starting Open5GS components..."
    echo "------------------------------"
    
    # Start NRF first (all other components depend on it)
    start_component "nrf" "127.0.0.3" "7777"
    wait_for_nrf
    
    # Start other components
    # AMF with SCTP port for gNodeB connection
    start_component "amf" "127.0.0.4" "38412 7777"
    
    # SMF and UPF (order matters - SMF before UPF)
    start_component "smf" "127.0.0.5" "7777 8805"
    start_component "upf" "127.0.0.10" "2152 8805"
    
    # Other NFs (order doesn't matter much)
    start_component "ausf" "127.0.0.9" "7777"
    start_component "udm" "127.0.0.8" "7777"
    start_component "udr" "127.0.0.7" "7777"
    start_component "pcf" "127.0.0.6" "7777"
    start_component "nssf" "127.0.0.11" "7777"
    start_component "bsf" "127.0.0.12" "7777"
    
    echo ""
    echo "=== Open5GS 5G Core Started Successfully ==="
    echo ""
    echo "Component Status:"
    echo "-----------------"
    ps aux | grep open5gs | grep -v grep | awk '{print "  " $11 " (PID: " $2 ")"}'
    
    echo ""
    echo "Network Configuration:"
    echo "---------------------"
    echo "  MongoDB:       127.0.0.2:27017"
    echo "  NRF:           127.0.0.3:7777"
    echo "  AMF (NGAP):    127.0.0.4:38412"
    echo "  AMF (SBI):     127.0.0.4:7777"
    echo "  SMF:           127.0.0.5:7777"
    echo "  UPF (GTP-U):   127.0.0.10:2152"
    echo "  UPF (PFCP):    127.0.0.10:8805"
    echo "  TUN Device:    ogstun (10.45.0.1/16)"
    echo ""
    echo "To connect a gNodeB:"
    echo "  - Configure N2 interface to connect to 127.0.0.4:38412"
    echo "  - Use PLMN: 00101 (MCC=001, MNC=01)"
    echo "  - Use TAC: 7"
    echo ""
    echo "Management commands:"
    echo "  - Stop:   ${SCRIPT_DIR}/stop-open5gs.sh"
    echo "  - Status: ${SCRIPT_DIR}/status-open5gs.sh"
    echo "  - Logs:   tail -f ${LOG_DIR}/<component>.log"
    echo ""
}

# Run main function
main "$@"