#!/bin/bash
# test_srsran.sh - Reference 5G SA test with srsRAN gNodeB + UE + Open5GS
# This is the DEFINITIVE test script using the proven 10MHz configuration
# Uses native Open5GS installation with multi-loopback configuration

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
log_debug() { echo -e "${BLUE}[DEBUG]${NC} $1"; }

# Create log directory
LOG_DIR="/workspace/logs/$(date +%Y%m%d_%H%M%S)_srsran"
mkdir -p "$LOG_DIR"

log_info "=== srsRAN Complete 5G SA Test ==="
log_info "Testing: Open5GS + srsRAN gNodeB + srsRAN UE"
log_info "Log directory: $LOG_DIR"

# Check if we're running inside the container
if [ -f /.dockerenv ]; then
    IN_DOCKER=1
    log_info "Running inside Docker container"
else
    log_info "Running test through Docker exec"
fi

# Comprehensive cleanup function
cleanup() {
    log_info "Performing comprehensive cleanup..."
    
    # Kill only gnb and srsue processes, leave Open5GS running
    for process in gnb srsue; do
        if [ "$IN_DOCKER" = "1" ]; then
            pkill -9 -f "$process" 2>/dev/null || true
        else
            docker exec albor-gnb-dev bash -c "pkill -9 -f '$process' 2>/dev/null || true" 2>/dev/null || true
        fi
    done
    
    # Clean up zombie processes
    if [ "$IN_DOCKER" = "1" ]; then
        ps aux | grep defunct | awk '{print $2}' | xargs -r kill -9 2>/dev/null || true
    else
        docker exec albor-gnb-dev bash -c "ps aux | grep defunct | awk '{print \$2}' | xargs -r kill -9 2>/dev/null || true" 2>/dev/null || true
    fi
    
    # Kill processes on ZMQ ports
    for port in 2000 2001; do
        if [ "$IN_DOCKER" = "1" ]; then
            PID=$(lsof -ti:$port 2>/dev/null || true)
            if [ ! -z "$PID" ]; then
                kill -9 $PID 2>/dev/null || true
            fi
        else
            docker exec albor-gnb-dev bash -c "
                PID=\$(lsof -ti:$port 2>/dev/null || true)
                if [ ! -z \"\$PID\" ]; then
                    kill -9 \$PID 2>/dev/null || true
                fi
            " 2>/dev/null || true
        fi
    done
    
    sleep 2
}

# Set trap for cleanup on exit
trap cleanup EXIT

# Initial cleanup
log_info "Initial cleanup of existing processes..."
cleanup

# Step 1: Setup loopback interfaces
log_info "Step 1: Setting up loopback interfaces..."
if [ "$IN_DOCKER" = "1" ]; then
    # Inside container - run directly
    if [ -x /workspace/scripts/open5gs/setup_loopback_interfaces.sh ]; then
        /workspace/scripts/open5gs/setup_loopback_interfaces.sh
    else
        log_warn "Loopback setup script not found, continuing anyway..."
    fi
else
    # Outside container - use docker exec
    docker exec albor-gnb-dev /workspace/scripts/open5gs/setup_loopback_interfaces.sh
fi

# Step 2: Start MongoDB
log_info "Step 2: Starting MongoDB..."
if [ "$IN_DOCKER" = "1" ]; then
    # Create MongoDB data directory if it doesn't exist
    mkdir -p /var/lib/mongodb
    mkdir -p /var/log/open5gs
    
    # Check if MongoDB is already running on 127.0.0.2
    if netstat -tuln 2>/dev/null | grep -q "127.0.0.2:27017"; then
        log_info "✓ MongoDB already running on 127.0.0.2:27017"
    else
        # Kill any MongoDB on localhost
        pkill -9 mongod 2>/dev/null || true
        sleep 2
        
        # Start MongoDB on loopback interface
        mongod --dbpath /var/lib/mongodb \
               --logpath "$LOG_DIR/mongodb.log" \
               --bind_ip 127.0.0.2 \
               --fork \
               --quiet
        
        # Wait for MongoDB to be ready
        for i in {1..20}; do
            if netstat -tuln 2>/dev/null | grep -q "127.0.0.2:27017"; then
                log_info "✓ MongoDB started on 127.0.0.2:27017"
                break
            fi
            if [ $i -eq 20 ]; then
                log_error "MongoDB failed to start"
                exit 1
            fi
            printf "\r[%02d/20] Waiting for MongoDB..." "$i"
            sleep 1
        done
        echo ""
    fi
else
    # Outside container - use docker exec
    if docker exec albor-gnb-dev netstat -tuln 2>/dev/null | grep -q "127.0.0.2:27017"; then
        log_info "✓ MongoDB already running on 127.0.0.2:27017"
    else
        docker exec albor-gnb-dev bash -c "
            mkdir -p /var/lib/mongodb /var/log/open5gs
            pkill -9 mongod 2>/dev/null || true
            sleep 2
            mongod --dbpath /var/lib/mongodb \
                   --logpath $LOG_DIR/mongodb.log \
                   --bind_ip 127.0.0.2 \
                   --fork \
                   --quiet
        "
        
        # Wait for MongoDB
        for i in {1..20}; do
            if docker exec albor-gnb-dev netstat -tuln 2>/dev/null | grep -q "127.0.0.2:27017"; then
                log_info "✓ MongoDB started on 127.0.0.2:27017"
                break
            fi
            if [ $i -eq 20 ]; then
                log_error "MongoDB failed to start"
                exit 1
            fi
            printf "\r[%02d/20] Waiting for MongoDB..." "$i"
            sleep 1
        done
        echo ""
    fi
fi

# Step 3: Start Open5GS components
log_info "Step 3: Starting Open5GS Core Network..."

# Use the existing start script which handles all components properly
if [ "$IN_DOCKER" = "1" ]; then
    # Run start script
    if [ -x /workspace/scripts/open5gs/start_open5gs_core.sh ]; then
        /workspace/scripts/open5gs/start_open5gs_core.sh > "$LOG_DIR/open5gs_startup.log" 2>&1
    else
        log_error "start_open5gs_core.sh not found!"
        exit 1
    fi
else
    # Outside container - use docker exec
    docker exec albor-gnb-dev /workspace/scripts/open5gs/start_open5gs_core.sh > /tmp/open5gs_startup.log 2>&1
fi

# Wait for AMF to be ready
log_info "Waiting for AMF to be ready..."
for i in {1..30}; do
    if [ "$IN_DOCKER" = "1" ]; then
        if netstat -tuln 2>/dev/null | grep -q "127.0.0.4:38412"; then
            log_info "✓ AMF is listening on 127.0.0.4:38412 (SCTP)"
            break
        fi
    else
        if docker exec albor-gnb-dev netstat -tuln 2>/dev/null | grep -q "127.0.0.4:38412"; then
            log_info "✓ AMF is listening on 127.0.0.4:38412 (SCTP)"
            break
        fi
    fi
    
    if [ $i -eq 30 ]; then
        log_error "AMF failed to start on 127.0.0.4:38412"
        log_info "Checking Open5GS startup log..."
        if [ "$IN_DOCKER" = "1" ]; then
            tail -20 "$LOG_DIR/open5gs_startup.log"
        else
            tail -20 /tmp/open5gs_startup.log
        fi
        exit 1
    fi
    
    printf "\r[%02d/30] Waiting for AMF SCTP interface..." "$i"
    sleep 1
done
echo ""

# Step 4: Verify test subscriber was added by cleanup_and_restart.sh
log_info "Step 4: Verifying test subscriber..."

# The cleanup_and_restart.sh script already adds the test subscriber
# Just verify it's there
if [ "$IN_DOCKER" = "1" ]; then
    if mongo 127.0.0.2:27017/open5gs --quiet --eval 'db.subscribers.findOne({"imsi": "001010000000001"})' | grep -q "001010000000001"; then
        log_info "✓ Test subscriber verified (IMSI: 001010000000001)"
    else
        log_warn "Test subscriber not found, adding it..."
        # Fallback: add subscriber if not present
        mongo 127.0.0.2:27017/open5gs --quiet --eval '
            db.subscribers.deleteOne({ "imsi": "001010000000001" });
            db.subscribers.insertOne({
                "imsi": "001010000000001",
                "msisdn": ["0000000001"],
                "imeisv": "353490069873310",
                "security": {
                    "k": "00112233445566778899AABBCCDDEEFF",
                    "opc": "63BFA50EE6523365FF14C1F45F88737D",
                    "amf": "8000"
                },
                "ambr": {
                    "downlink": { "value": 1, "unit": 3 },
                    "uplink": { "value": 1, "unit": 3 }
                },
                "slice": [{
                    "sst": 1,
                    "default_indicator": true,
                    "session": [{
                        "name": "internet",
                        "type": 3,
                        "qos": { "index": 9 },
                        "ambr": {
                            "downlink": { "value": 1, "unit": 3 },
                            "uplink": { "value": 1, "unit": 3 }
                        }
                    }]
                }],
                "access_restriction_data": 32,
                "subscribed_rau_tau_timer": 12,
                "network_access_mode": 0
            });'
    fi
else
    docker exec albor-gnb-dev bash -c "mongo 127.0.0.2:27017/open5gs --quiet --eval 'db.subscribers.findOne({\"imsi\": \"001010000000001\"})' | grep -q '001010000000001' && echo '✓ Test subscriber verified'"
fi

# Give Open5GS time to stabilize
sleep 3

# Step 5: Configure and start gNodeB
log_info "Step 5: Starting srsRAN gNodeB..."

# Create a temporary config based on our definitive config with log paths updated
cp /workspace/config/srsran_gnb/gnb_zmq_10mhz.yml /tmp/gnb_config.yml

# Update log and pcap paths in the config
sed -i "s|filename: /tmp/gnb.log|filename: $LOG_DIR/gnb.log|g" /tmp/gnb_config.yml
sed -i "s|mac_filename: /tmp/gnb_mac.pcap|mac_filename: $LOG_DIR/gnb_mac.pcap|g" /tmp/gnb_config.yml
sed -i "s|ngap_filename: /tmp/gnb_ngap.pcap|ngap_filename: $LOG_DIR/gnb_ngap.pcap|g" /tmp/gnb_config.yml

# Update bind addresses for multi-loopback setup
sed -i "s|bind_addr: 127.0.0.1|bind_addr: 127.0.0.11|g" /tmp/gnb_config.yml
sed -i "s|gtpu_bind_addr: 127.0.0.1|gtpu_bind_addr: 127.0.0.11|g" /tmp/gnb_config.yml
sed -i "s|gtpu_ext_addr: 127.0.0.1|gtpu_ext_addr: 127.0.0.11|g" /tmp/gnb_config.yml

# Copy config if not running inside container
if [ "$IN_DOCKER" != "1" ]; then
    docker cp /tmp/gnb_config.yml albor-gnb-dev:/tmp/gnb_config.yml
fi

# Start gNodeB
if [ "$IN_DOCKER" = "1" ]; then
    cd /opt/srsran_project
    /opt/srsran_project/bin/gnb -c /tmp/gnb_config.yml > $LOG_DIR/gnb_stdout.log 2>&1 &
    GNB_PID=$!
else
    docker exec albor-gnb-dev bash -c "
        cd /opt/srsran_project
        /opt/srsran_project/bin/gnb -c /tmp/gnb_config.yml > $LOG_DIR/gnb_stdout.log 2>&1 &
        echo \$!
    " > /tmp/gnb_pid.txt
    GNB_PID=$(cat /tmp/gnb_pid.txt)
fi

log_info "gNodeB started (PID: $GNB_PID)"

# Wait for AMF connection
log_info "Waiting for gNodeB to connect to AMF..."
for i in {1..30}; do
    if [ "$IN_DOCKER" = "1" ]; then
        if grep -q "NG setup procedure completed" "$LOG_DIR/gnb.log" 2>/dev/null || \
           grep -q "NG setup procedure completed" "$LOG_DIR/gnb_stdout.log" 2>/dev/null; then
            log_info "✓ gNodeB connected to AMF successfully!"
            break
        fi
    else
        if docker exec albor-gnb-dev bash -c "grep -q 'NG setup procedure completed' '$LOG_DIR/gnb.log' 2>/dev/null || grep -q 'NG setup procedure completed' '$LOG_DIR/gnb_stdout.log' 2>/dev/null"; then
            log_info "✓ gNodeB connected to AMF successfully!"
            break
        fi
    fi
    
    if [ $i -eq 30 ]; then
        log_error "✗ gNodeB failed to connect to AMF"
        log_info "gNodeB log tail:"
        if [ "$IN_DOCKER" = "1" ]; then
            tail -30 "$LOG_DIR/gnb.log" 2>/dev/null || tail -30 "$LOG_DIR/gnb_stdout.log"
        else
            docker exec albor-gnb-dev bash -c "tail -30 '$LOG_DIR/gnb.log' 2>/dev/null || tail -30 '$LOG_DIR/gnb_stdout.log'"
        fi
        exit 1
    fi
    
    printf "\r[%02d/30] Waiting for NG setup completion..." "$i"
    sleep 1
done
echo ""

# Give gNodeB time to stabilize
sleep 3

# Step 6: Start srsUE with correct command line parameters
log_info "Step 6: Starting srsUE with NR configuration..."

# Start srsUE with command line parameters for NR ARFCN
if [ "$IN_DOCKER" = "1" ]; then
    cd /opt/srsran
    export LD_LIBRARY_PATH=/opt/srsran/lib:$LD_LIBRARY_PATH
    
    # Create a temporary UE config with updated log paths
    cp /workspace/config/srsue/ue_nr_zmq_10mhz.conf /tmp/ue_config.conf
    sed -i "s|filename = /tmp/ue.log|filename = $LOG_DIR/ue.log|g" /tmp/ue_config.conf
    sed -i "s|mac_filename = /tmp/ue_mac.pcap|mac_filename = $LOG_DIR/ue_mac.pcap|g" /tmp/ue_config.conf
    sed -i "s|mac_nr_filename = /tmp/ue_mac_nr.pcap|mac_nr_filename = $LOG_DIR/ue_mac_nr.pcap|g" /tmp/ue_config.conf
    sed -i "s|nas_filename = /tmp/ue_nas.pcap|nas_filename = $LOG_DIR/ue_nas.pcap|g" /tmp/ue_config.conf
    
    # Use the definitive 10 MHz configuration with command line parameters
    /opt/srsran/bin/srsue \
        /tmp/ue_config.conf \
        --rat.nr.dl_nr_arfcn 368500 \
        --rat.nr.ssb_nr_arfcn 368410 \
        --rat.nr.nof_prb 52 \
        --rat.nr.scs 15 \
        --rat.nr.ssb_scs 15 \
        > $LOG_DIR/ue_stdout.log 2>&1 &
    UE_PID=$!
else
    docker exec albor-gnb-dev bash -c "
        cd /opt/srsran
        export LD_LIBRARY_PATH=/opt/srsran/lib:\$LD_LIBRARY_PATH
        
        # Create a temporary UE config with updated log paths
        cp /workspace/config/srsue/ue_nr_zmq_10mhz.conf /tmp/ue_config.conf
        sed -i "s|filename = /tmp/ue.log|filename = $LOG_DIR/ue.log|g" /tmp/ue_config.conf
        sed -i "s|mac_filename = /tmp/ue_mac.pcap|mac_filename = $LOG_DIR/ue_mac.pcap|g" /tmp/ue_config.conf
        sed -i "s|mac_nr_filename = /tmp/ue_mac_nr.pcap|mac_nr_filename = $LOG_DIR/ue_mac_nr.pcap|g" /tmp/ue_config.conf
        sed -i "s|nas_filename = /tmp/ue_nas.pcap|nas_filename = $LOG_DIR/ue_nas.pcap|g" /tmp/ue_config.conf
        
        /opt/srsran/bin/srsue \
            /tmp/ue_config.conf \
            --rat.nr.dl_nr_arfcn 368500 \
            --rat.nr.ssb_nr_arfcn 368410 \
            --rat.nr.nof_prb 52 \
            --rat.nr.scs 15 \
            --rat.nr.ssb_scs 15 \
            > $LOG_DIR/ue_stdout.log 2>&1 &
        echo \$!
    " > /tmp/ue_pid.txt
    UE_PID=$(cat /tmp/ue_pid.txt)
fi

log_info "srsUE started (PID: $UE_PID)"

# Step 7: Monitor registration
log_info "Step 7: Monitoring 5G registration..."

TIMEOUT=60
SUCCESS=false
MILESTONES=()

# Helper function to check logs
check_log() {
    local pattern="$1"
    local file="$2"
    if [ "$IN_DOCKER" = "1" ]; then
        grep -q "$pattern" "$file" 2>/dev/null
    else
        docker exec albor-gnb-dev grep -q "$pattern" "$file" 2>/dev/null
    fi
}

# Monitor registration progress
for i in $(seq 1 $TIMEOUT); do
    printf "\r[%02d/%02d] Checking registration status..." "$i" "$TIMEOUT"
    
    # Check UE log file (from config)
    UE_LOG="$LOG_DIR/ue.log"
    
    # Check various stages
    if check_log "Found Cell.*PCI=1" "$UE_LOG" || check_log "Found Cell.*PCI=1" "$LOG_DIR/ue_stdout.log"; then
        if [[ ! " ${MILESTONES[@]} " =~ " cell_found " ]]; then
            echo ""
            log_info "✓ UE found cell (PCI=1)"
            MILESTONES+=("cell_found")
        fi
    fi
    
    if check_log "Random Access Complete" "$UE_LOG" || check_log "Random Access Complete" "$LOG_DIR/ue_stdout.log"; then
        if [[ ! " ${MILESTONES[@]} " =~ " rach_complete " ]]; then
            echo ""
            log_info "✓ Random access completed"
            MILESTONES+=("rach_complete")
        fi
    fi
    
    if check_log "RRC Connected" "$UE_LOG" || check_log "RRC Connected" "$LOG_DIR/ue_stdout.log"; then
        if [[ ! " ${MILESTONES[@]} " =~ " rrc_connected " ]]; then
            echo ""
            log_info "✓ RRC connected"
            MILESTONES+=("rrc_connected")
        fi
    fi
    
    # Check for 5G NAS registration
    if check_log "NAS-5G.*Registration complete" "$UE_LOG" || \
       check_log "NAS-5G.*Registration complete" "$LOG_DIR/ue_stdout.log" || \
       check_log "EMM-REGISTERED" "$UE_LOG" || \
       check_log "EMM-REGISTERED" "$LOG_DIR/ue_stdout.log"; then
        SUCCESS=true
        echo ""
        log_info "✓ 5G NAS registration complete!"
        break
    fi
    
    # Also check AMF logs for registration
    if [ "$IN_DOCKER" = "1" ]; then
        if grep -q "5GMM-REGISTERED" "/var/log/open5gs/amf.log" 2>/dev/null; then
            SUCCESS=true
            echo ""
            log_info "✓ AMF confirms 5G registration!"
            break
        fi
    else
        if docker exec albor-gnb-dev grep -q "5GMM-REGISTERED" "/var/log/open5gs/amf.log" 2>/dev/null; then
            SUCCESS=true
            echo ""
            log_info "✓ AMF confirms 5G registration!"
            break
        fi
    fi
    
    sleep 1
done

echo ""
echo "=========================================="
log_info "TEST RESULTS:"
echo "=========================================="

# Helper function to show logs
show_log() {
    local file="$1"
    local lines="${2:-20}"
    if [ "$IN_DOCKER" = "1" ]; then
        if [ -f "$file" ]; then
            tail -$lines "$file" 2>/dev/null || echo "  (log file not found)"
        else
            echo "  (log file not found: $file)"
        fi
    else
        docker exec albor-gnb-dev bash -c "
            if [ -f '$file' ]; then
                tail -$lines '$file' 2>/dev/null || echo '  (log file not found)'
            else
                echo '  (log file not found: $file)'
            fi
        "
    fi
}

if [ "$SUCCESS" = "true" ]; then
    log_info "✅ SUCCESS: UE registered to 5G network!"
    
    # Show registration milestones
    echo ""
    log_info "Registration milestones achieved:"
    for milestone in "${MILESTONES[@]}"; do
        case $milestone in
            cell_found) echo "  ✓ Cell detection" ;;
            rach_complete) echo "  ✓ Random access procedure" ;;
            rrc_connected) echo "  ✓ RRC connection establishment" ;;
        esac
    done
    echo "  ✓ 5G NAS registration"
    
    # Show key logs
    echo ""
    log_info "AMF registration logs:"
    if [ "$IN_DOCKER" = "1" ]; then
        grep -E "(5GMM-REGISTERED|InitialUEMessage|PDU Session)" "/var/log/open5gs/amf.log" 2>/dev/null | tail -5 || echo "  (no registration logs found)"
    else
        docker exec albor-gnb-dev sh -c "grep -E '(5GMM-REGISTERED|InitialUEMessage|PDU Session)' '/var/log/open5gs/amf.log' 2>/dev/null | tail -5" || echo "  (no registration logs found)"
    fi
    
else
    log_error "❌ FAILED: UE did not register within ${TIMEOUT} seconds"
    
    # Debug information
    echo ""
    log_info "Debug information:"
    
    echo ""
    log_info "Milestones achieved:"
    if [ ${#MILESTONES[@]} -eq 0 ]; then
        echo "  ✗ No milestones achieved"
    else
        for milestone in "${MILESTONES[@]}"; do
            case $milestone in
                cell_found) echo "  ✓ Cell detection" ;;
                rach_complete) echo "  ✓ Random access procedure" ;;
                rrc_connected) echo "  ✓ RRC connection establishment" ;;
            esac
        done
    fi
    
    echo ""
    log_info "gNodeB log tail:"
    show_log "$LOG_DIR/gnb.log" 30
    
    echo ""
    log_info "UE log tail:"
    show_log "$LOG_DIR/ue.log" 30
    
    echo ""
    log_info "AMF log tail:"
    show_log "/var/log/open5gs/amf.log" 20
    
    echo ""
    log_info "Process status:"
    if [ "$IN_DOCKER" = "1" ]; then
        ps aux | grep -E "(gnb|srsue|open5gs)" | grep -v grep || echo "  No relevant processes found"
    else
        docker exec albor-gnb-dev bash -c "ps aux | grep -E '(gnb|srsue|open5gs)' | grep -v grep" || echo "  No relevant processes found"
    fi
fi

echo "=========================================="

# Keep running if successful for monitoring
if [ "$SUCCESS" = "true" ]; then
    log_info "System is running. Press Ctrl+C to stop."
    log_info "Monitor logs in: $LOG_DIR"
    
    # Wait for user interrupt
    wait
fi