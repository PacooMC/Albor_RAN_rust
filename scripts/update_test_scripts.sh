#!/bin/bash
# update_test_scripts.sh - Update test scripts to use proper container capabilities

set -e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

log_info() { echo -e "${GREEN}[INFO]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }
log_warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }

log_info "=== Updating Test Scripts for Container Capabilities ==="

# Create a wrapper script that ensures container runs with proper capabilities
cat > scripts/docker_run_with_caps.sh << 'EOF'
#!/bin/bash
# docker_run_with_caps.sh - Run container with proper capabilities for Open5GS

# Check if container exists
if docker ps -a | grep -q albor-gnb-dev; then
    # Check if it's running
    if docker ps | grep -q albor-gnb-dev; then
        echo "Container albor-gnb-dev is already running"
    else
        echo "Starting existing container albor-gnb-dev..."
        docker start albor-gnb-dev
    fi
else
    echo "Creating new container albor-gnb-dev with capabilities..."
    docker run -d \
        --name albor-gnb-dev \
        --cap-add=NET_ADMIN \
        --cap-add=SYS_ADMIN \
        --cap-add=NET_RAW \
        --cap-add=NET_BIND_SERVICE \
        --privileged \
        -v $(pwd):/workspace \
        -w /workspace \
        albor-gnb-dev \
        tail -f /dev/null
fi

# Ensure the container has the required capabilities
docker exec albor-gnb-dev capsh --print | grep -q "cap_net_admin" || {
    echo "ERROR: Container doesn't have NET_ADMIN capability!"
    echo "Recreating container with proper capabilities..."
    docker stop albor-gnb-dev
    docker rm albor-gnb-dev
    docker run -d \
        --name albor-gnb-dev \
        --cap-add=NET_ADMIN \
        --cap-add=SYS_ADMIN \
        --cap-add=NET_RAW \
        --cap-add=NET_BIND_SERVICE \
        --privileged \
        -v $(pwd):/workspace \
        -w /workspace \
        albor-gnb-dev \
        tail -f /dev/null
}

echo "Container albor-gnb-dev is ready with proper capabilities"
EOF

chmod +x scripts/docker_run_with_caps.sh

# Update quicktest.sh to ensure container has proper capabilities
log_info "Updating quicktest.sh to check container capabilities..."

# Create a backup
cp quicktest.sh quicktest.sh.bak

# Insert capability check at the beginning of quicktest.sh
cat > /tmp/quicktest_header.sh << 'EOF'
#!/bin/bash
# quicktest.sh - Quick test runner for Albor gNodeB

# Ensure container has proper capabilities
if [ ! -f /.dockerenv ]; then
    # Running outside container - ensure container has capabilities
    if docker ps | grep -q albor-gnb-dev; then
        # Check if container has NET_ADMIN capability
        if ! docker exec albor-gnb-dev capsh --print 2>/dev/null | grep -q "cap_net_admin"; then
            echo "WARNING: Container doesn't have NET_ADMIN capability!"
            echo "Please restart the container with: ./scripts/docker_run_with_caps.sh"
            exit 1
        fi
    else
        echo "Container not running. Starting with proper capabilities..."
        ./scripts/docker_run_with_caps.sh
    fi
fi

EOF

# Get the rest of quicktest.sh (excluding shebang)
tail -n +2 quicktest.sh.bak > /tmp/quicktest_body.sh

# Combine them
cat /tmp/quicktest_header.sh /tmp/quicktest_body.sh > quicktest.sh
chmod +x quicktest.sh

log_info "✓ quicktest.sh updated with capability checks"

# Create setup script for loopback interfaces with proper error handling
cat > scripts/setup_loopback_robust.sh << 'EOF'
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
EOF

chmod +x scripts/setup_loopback_robust.sh

# Update native Open5GS startup script to use robust loopback setup
if [ -f config/open5gs_native/start_open5gs.sh ]; then
    log_info "Updating native Open5GS startup script..."
    
    # Replace the loopback check section
    sed -i 's|if ! ip addr show lo2|if ! ip addr show lo | grep -q "127.0.0.2"|' config/open5gs_native/start_open5gs.sh
    sed -i 's|log_error "Loopback interfaces not found.*"|/workspace/scripts/setup_loopback_robust.sh|' config/open5gs_native/start_open5gs.sh
fi

log_info ""
log_info "✅ Test script updates complete!"
log_info ""
log_info "To run tests with proper capabilities:"
log_info "1. Start container: ./scripts/docker_run_with_caps.sh"
log_info "2. Run tests: ./quicktest.sh"
log_info ""
log_info "The container will now have:"
log_info "  - NET_ADMIN capability for network configuration"
log_info "  - Ability to create loopback interfaces"
log_info "  - Proper permissions for Open5GS operation"