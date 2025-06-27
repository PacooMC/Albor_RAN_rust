#!/bin/bash
# Check SCTP support and setup appropriate solution for Open5GS testing
# This script determines if SCTP is available and configures the environment accordingly

set -e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

log_info() { echo -e "${GREEN}[INFO]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }
log_warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }

# Check if running in Docker
IN_DOCKER=0
if [ -f /.dockerenv ]; then
    IN_DOCKER=1
fi

log_info "=== SCTP Support Check and Setup ==="

# Function to check SCTP support
check_sctp_support() {
    # Method 1: Check if SCTP module is loaded
    if lsmod 2>/dev/null | grep -q sctp; then
        log_info "✓ SCTP kernel module is loaded"
        return 0
    fi
    
    # Method 2: Try to create SCTP socket using Python
    if command -v python3 >/dev/null 2>&1; then
        local result=$(python3 -c "
import socket
try:
    s = socket.socket(socket.AF_INET, socket.SOCK_STREAM, socket.IPPROTO_SCTP)
    s.close()
    print('SUPPORTED')
except:
    print('NOT_SUPPORTED')
" 2>/dev/null)
        
        if [ "$result" = "SUPPORTED" ]; then
            log_info "✓ SCTP socket creation successful"
            return 0
        fi
    fi
    
    # Method 3: Check with checksctp tool
    if command -v checksctp >/dev/null 2>&1; then
        if checksctp 2>/dev/null | grep -q "SCTP supported"; then
            log_info "✓ checksctp confirms SCTP support"
            return 0
        fi
    fi
    
    return 1
}

# Function to try loading SCTP module
try_load_sctp() {
    if [ "$IN_DOCKER" = "1" ]; then
        log_warn "Cannot load kernel modules inside Docker container"
        return 1
    fi
    
    log_info "Attempting to load SCTP kernel module..."
    if sudo modprobe sctp 2>/dev/null; then
        log_info "✓ SCTP module loaded successfully"
        return 0
    else
        log_error "✗ Failed to load SCTP module"
        return 1
    fi
}

# Function to install SCTP packages
install_sctp_packages() {
    log_info "Installing SCTP development packages..."
    
    if [ "$IN_DOCKER" = "1" ]; then
        apt-get update >/dev/null 2>&1
        apt-get install -y libsctp-dev lksctp-tools >/dev/null 2>&1 || {
            log_warn "Failed to install SCTP packages"
            return 1
        }
    else
        if command -v apt-get >/dev/null 2>&1; then
            sudo apt-get update >/dev/null 2>&1
            sudo apt-get install -y libsctp-dev lksctp-tools >/dev/null 2>&1 || {
                log_warn "Failed to install SCTP packages"
                return 1
            }
        else
            log_warn "Package manager not supported, please install libsctp-dev and lksctp-tools manually"
            return 1
        fi
    fi
    
    log_info "✓ SCTP packages installed"
    return 0
}

# Main logic
SCTP_AVAILABLE=false
SOLUTION=""

# Step 1: Check current SCTP support
log_info "Step 1: Checking SCTP support..."
if check_sctp_support; then
    SCTP_AVAILABLE=true
    SOLUTION="native"
else
    log_warn "✗ SCTP is not currently available"
    
    # Step 2: Try to enable SCTP
    if [ "$IN_DOCKER" = "0" ]; then
        log_info "Step 2: Trying to enable SCTP on host..."
        if try_load_sctp && check_sctp_support; then
            SCTP_AVAILABLE=true
            SOLUTION="native"
        fi
    else
        log_info "Step 2: Installing SCTP libraries in container..."
        install_sctp_packages
        
        # Check if Docker has required capabilities
        if [ -f /proc/self/status ]; then
            if grep -q "CapEff:.*40000000" /proc/self/status 2>/dev/null; then
                log_info "Container has CAP_NET_ADMIN capability"
                if check_sctp_support; then
                    SCTP_AVAILABLE=true
                    SOLUTION="native"
                fi
            else
                log_warn "Container lacks required capabilities for SCTP"
            fi
        fi
    fi
fi

# Step 3: Determine solution
log_info "Step 3: Determining solution..."

if [ "$SCTP_AVAILABLE" = "true" ]; then
    log_info "✅ SCTP is available - can use native Open5GS"
    SOLUTION="native"
else
    log_warn "❌ SCTP is not available in this environment"
    
    if [ "$IN_DOCKER" = "1" ]; then
        log_info "Running in Docker without SCTP support"
        log_info "Available solutions:"
        log_info "  1. Use Mock AMF (TCP-based) for basic testing"
        log_info "  2. Install Open5GS on host system"
        log_info "  3. Run Docker with --privileged flag (if allowed)"
        SOLUTION="mock"
    else
        log_info "Running on host - you can install Open5GS directly"
        SOLUTION="host"
    fi
fi

# Step 4: Provide solution setup
echo ""
log_info "=== Solution Setup ==="

case "$SOLUTION" in
    "native")
        log_info "You can use Open5GS with native SCTP support"
        log_info "No additional configuration needed"
        ;;
        
    "mock")
        log_info "Setting up Mock AMF for testing..."
        
        # Make mock_amf.py executable
        if [ -f /workspace/scripts/mock_amf.py ]; then
            chmod +x /workspace/scripts/mock_amf.py
            log_info "✓ Mock AMF script is ready"
            log_info ""
            log_info "To start Mock AMF:"
            log_info "  python3 /workspace/scripts/mock_amf.py"
            log_info ""
            log_info "Note: This provides basic NGAP connectivity for testing"
            log_info "      but does not implement full AMF functionality"
        else
            log_error "Mock AMF script not found at /workspace/scripts/mock_amf.py"
        fi
        ;;
        
    "host")
        log_info "Install Open5GS on your host system:"
        log_info ""
        log_info "For Ubuntu/Debian:"
        log_info "  sudo apt update"
        log_info "  sudo apt install software-properties-common"
        log_info "  sudo add-apt-repository ppa:open5gs/latest"
        log_info "  sudo apt update"
        log_info "  sudo apt install open5gs"
        log_info ""
        log_info "Then configure AMF to listen on accessible address:"
        log_info "  Edit /etc/open5gs/amf.yaml"
        log_info "  Set ngap addr to 0.0.0.0 or specific interface IP"
        ;;
esac

# Step 5: Create environment file for other scripts
ENV_FILE="/tmp/sctp_env.sh"
cat > "$ENV_FILE" << EOF
# SCTP Environment Configuration
# Generated by check_and_setup_sctp.sh
export SCTP_AVAILABLE=$SCTP_AVAILABLE
export SCTP_SOLUTION="$SOLUTION"
export IN_DOCKER=$IN_DOCKER
EOF

log_info ""
log_info "Environment saved to: $ENV_FILE"
log_info "Source this file in other scripts: source $ENV_FILE"

# Summary
echo ""
echo "=========================================="
log_info "Summary:"
echo "=========================================="
echo "SCTP Available: $SCTP_AVAILABLE"
echo "Solution: $SOLUTION"
echo "Environment: $([ "$IN_DOCKER" = "1" ] && echo "Docker" || echo "Host")"
echo "=========================================="

# Exit with appropriate code
if [ "$SCTP_AVAILABLE" = "true" ] || [ "$SOLUTION" != "" ]; then
    exit 0
else
    exit 1
fi