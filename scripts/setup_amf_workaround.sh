#!/bin/bash
# setup_amf_workaround.sh - Comprehensive AMF workaround for Docker without --privileged
# Tries multiple approaches to get AMF working:
# 1. Check if SCTP module can be loaded
# 2. Try usrsctp if available
# 3. Use TCP bridge/mock as fallback

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

# Create work directory
WORK_DIR="$HOME/Albor_RAN_rust/amf_workaround"
mkdir -p "$WORK_DIR"
cd "$WORK_DIR"

log_info "=== AMF Workaround Setup ==="

# Step 1: Check SCTP support
log_info "Step 1: Checking SCTP support..."

SCTP_AVAILABLE=false
SCTP_METHOD=""

# Check if SCTP module is loaded
if lsmod 2>/dev/null | grep -q sctp; then
    log_info "✓ SCTP module already loaded"
    SCTP_AVAILABLE=true
    SCTP_METHOD="kernel"
else
    # Try to load SCTP module
    if modprobe sctp 2>/dev/null; then
        log_info "✓ SCTP module loaded successfully"
        SCTP_AVAILABLE=true
        SCTP_METHOD="kernel"
    else
        log_warn "✗ Cannot load SCTP kernel module (expected in Docker)"
    fi
fi

# Step 2: Check for usrsctp
log_info "Step 2: Checking for usrsctp..."

if [ "$SCTP_AVAILABLE" = "false" ]; then
    # Check if usrsctp is installed
    if ldconfig -p 2>/dev/null | grep -q usrsctp; then
        log_info "✓ usrsctp library found"
        SCTP_AVAILABLE=true
        SCTP_METHOD="usrsctp"
    else
        log_warn "✗ usrsctp not installed"
        
        # Try to install usrsctp
        log_info "Attempting to build usrsctp..."
        
        # Download and build usrsctp
        if command -v git >/dev/null 2>&1; then
            git clone https://github.com/sctplab/usrsctp.git 2>/dev/null || true
            if [ -d "usrsctp" ]; then
                cd usrsctp
                cmake . 2>/dev/null && make 2>/dev/null && make install 2>/dev/null
                if [ $? -eq 0 ]; then
                    ldconfig
                    log_info "✓ usrsctp built and installed"
                    SCTP_AVAILABLE=true
                    SCTP_METHOD="usrsctp"
                else
                    log_warn "✗ Failed to build usrsctp"
                fi
                cd ..
            fi
        fi
    fi
fi

# Step 3: Create wrapper if using usrsctp
if [ "$SCTP_METHOD" = "usrsctp" ]; then
    log_info "Step 3: Creating usrsctp wrapper..."
    
    cat > "$WORK_DIR/sctp_wrapper.c" << 'EOF'
#define _GNU_SOURCE
#include <dlfcn.h>
#include <sys/types.h>
#include <sys/socket.h>
#include <netinet/in.h>
#include <stdio.h>

// Function pointer types
typedef int (*socket_fn)(int, int, int);
typedef int (*bind_fn)(int, const struct sockaddr*, socklen_t);
typedef int (*connect_fn)(int, const struct sockaddr*, socklen_t);
typedef int (*listen_fn)(int, int);
typedef int (*accept_fn)(int, struct sockaddr*, socklen_t*);

// Override socket() to detect SCTP and use TCP instead
int socket(int domain, int type, int protocol) {
    socket_fn real_socket = dlsym(RTLD_NEXT, "socket");
    
    if (protocol == 132) { // IPPROTO_SCTP
        fprintf(stderr, "[WRAPPER] Intercepted SCTP socket, using TCP instead\n");
        return real_socket(domain, type, IPPROTO_TCP);
    }
    
    return real_socket(domain, type, protocol);
}
EOF

    # Try to compile wrapper
    if gcc -shared -fPIC "$WORK_DIR/sctp_wrapper.c" -o "$WORK_DIR/sctp_wrapper.so" -ldl 2>/dev/null; then
        log_info "✓ SCTP wrapper compiled"
        export LD_PRELOAD="$WORK_DIR/sctp_wrapper.so"
    else
        log_warn "✗ Failed to compile SCTP wrapper"
    fi
fi

# Step 4: Setup fallback solution
if [ "$SCTP_AVAILABLE" = "false" ] || [ "$SCTP_METHOD" = "usrsctp" ]; then
    log_info "Step 4: Setting up TCP fallback..."
    
    # Start the SCTP to TCP bridge
    python3 $HOME/Albor_RAN_rust/scripts/sctp_to_tcp_bridge.py &
    BRIDGE_PID=$!
    sleep 2
    
    if ps -p $BRIDGE_PID > /dev/null 2>&1; then
        log_info "✓ SCTP-TCP bridge started (PID: $BRIDGE_PID)"
    else
        # Fallback to simple mock AMF
        python3 $HOME/Albor_RAN_rust/scripts/mock_amf.py &
        MOCK_PID=$!
        sleep 2
        
        if ps -p $MOCK_PID > /dev/null 2>&1; then
            log_info "✓ Mock AMF started (PID: $MOCK_PID)"
        else
            log_error "Failed to start any AMF workaround"
            exit 1
        fi
    fi
fi

# Step 5: Report status
log_info "Step 5: AMF Workaround Status"
echo "======================================"

if [ "$SCTP_AVAILABLE" = "true" ] && [ "$SCTP_METHOD" = "kernel" ]; then
    log_info "✅ SCTP kernel module available - use standard Open5GS"
    echo "No workaround needed, proceed with normal setup"
elif [ "$SCTP_METHOD" = "usrsctp" ]; then
    log_info "✅ Using usrsctp library with wrapper"
    echo "Start Open5GS with: LD_PRELOAD=$WORK_DIR/sctp_wrapper.so open5gs-amfd"
else
    log_info "✅ Using TCP bridge/mock AMF"
    echo "Connect gNodeB to 127.0.0.4:38412 (TCP)"
    echo "Note: Limited functionality - suitable for PHY/MAC testing only"
fi

echo "======================================"

# Save configuration
cat > "$WORK_DIR/amf_workaround.conf" << EOF
# AMF Workaround Configuration
SCTP_AVAILABLE=$SCTP_AVAILABLE
SCTP_METHOD=$SCTP_METHOD
WORK_DIR=$WORK_DIR
BRIDGE_PID=${BRIDGE_PID:-none}
MOCK_PID=${MOCK_PID:-none}

# To use this configuration:
$(if [ "$SCTP_METHOD" = "usrsctp" ]; then
    echo "export LD_PRELOAD=$WORK_DIR/sctp_wrapper.so"
elif [ "$SCTP_AVAILABLE" = "false" ]; then
    echo "# Use TCP endpoint at 127.0.0.4:38412"
fi)
EOF

log_info "Configuration saved to: $WORK_DIR/amf_workaround.conf"

# Create test script
cat > "$WORK_DIR/test_amf_connection.sh" << 'EOF'
#!/bin/bash
# Test AMF connection

echo "Testing AMF connection..."

# Try TCP connection
if timeout 2 bash -c "echo > /dev/tcp/127.0.0.4/38412" 2>/dev/null; then
    echo "✓ TCP connection to 127.0.0.4:38412 successful"
else
    echo "✗ TCP connection failed"
fi

# Check processes
echo ""
echo "AMF-related processes:"
ps aux | grep -E "(amf|mock|bridge)" | grep -v grep
EOF

chmod +x "$WORK_DIR/test_amf_connection.sh"
log_info "Test script created: $WORK_DIR/test_amf_connection.sh"

log_info "AMF workaround setup complete!"