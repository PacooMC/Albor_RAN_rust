#!/bin/bash
# setup_network_loopback.sh - Create multiple loopback interfaces for 5G SA testing
# This solves the GTP-U port 2152 conflict between gNodeB and UPF

set -e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

log_info() { echo -e "${GREEN}[INFO]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }
log_warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }

# Check if running as root (required for network interface creation)
if [ "$EUID" -ne 0 ]; then 
    log_error "This script must be run as root or with sudo"
    exit 1
fi

log_info "=== Setting up Multiple Loopback Interfaces for 5G SA ==="
log_info "This allows gNodeB and Open5GS to run without port conflicts"

# Function to create loopback interface
create_loopback() {
    local IP=$1
    local IFACE="lo$IP"
    
    # Check if interface already exists
    if ip link show $IFACE &>/dev/null; then
        log_warn "Interface $IFACE already exists, skipping creation"
    else
        log_info "Creating interface $IFACE with IP 127.0.0.$IP"
        ip link add name $IFACE type dummy
        ip addr add 127.0.0.$IP/24 dev $IFACE
        ip link set $IFACE up
    fi
}

# Create loopback interfaces for each component
# Following srsRAN tutorial approach:
# 127.0.0.2-10: Open5GS components
# 127.0.0.11: gNodeB
# 127.0.0.12-20: Reserved for future use

log_info "Creating loopback interfaces..."

# Open5GS components
create_loopback 2   # MongoDB
create_loopback 3   # NRF
create_loopback 4   # AMF
create_loopback 5   # SMF  
create_loopback 6   # AUSF
create_loopback 7   # UDM
create_loopback 8   # PCF
create_loopback 9   # UDR
create_loopback 10  # UPF (critical - needs port 2152)

# gNodeB
create_loopback 11  # gNodeB (also needs port 2152, but different IP)

# Reserved for future components
for i in {12..20}; do
    create_loopback $i
done

log_info "Verifying network interfaces..."

# Display created interfaces
echo ""
echo "Created interfaces:"
ip addr show | grep -E "lo[0-9]+:|127\.0\.0\.[0-9]+" | grep -v "127.0.0.1"

echo ""
log_info "Network setup complete!"
log_info ""
log_info "IP Address Assignment:"
log_info "  127.0.0.2  - MongoDB"
log_info "  127.0.0.3  - NRF (Network Repository Function)"
log_info "  127.0.0.4  - AMF (Access and Mobility Management)"
log_info "  127.0.0.5  - SMF (Session Management Function)"
log_info "  127.0.0.6  - AUSF (Authentication Server Function)"
log_info "  127.0.0.7  - UDM (Unified Data Management)"
log_info "  127.0.0.8  - PCF (Policy Control Function)"
log_info "  127.0.0.9  - UDR (Unified Data Repository)"
log_info "  127.0.0.10 - UPF (User Plane Function) - GTP-U port 2152"
log_info "  127.0.0.11 - gNodeB - GTP-U port 2152"
log_info ""
log_info "Both UPF and gNodeB can now bind to port 2152 on different IPs!"