#!/bin/bash
# 5G SA Complete Setup Test Script
# This script tests the complete 5G SA setup with Open5GS, srsRAN gNodeB, and srsUE
# 
# Prerequisites:
# - Docker and docker-compose installed
# - srsRAN Project and srsRAN 4G built and available
#
# Usage: ./test_5g_sa_setup.sh [--use-our-gnb]
#   --use-our-gnb: Use our Albor GNodeB implementation instead of srsRAN Project

set -e  # Exit on error

# Parse command line arguments
USE_OUR_GNB=false
while [[ $# -gt 0 ]]; do
    case $1 in
        --use-our-gnb)
            USE_OUR_GNB=true
            shift
            ;;
        *)
            echo "Unknown option: $1"
            echo "Usage: $0 [--use-our-gnb]"
            exit 1
            ;;
    esac
done

# Color codes for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Logging functions
log_info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

log_debug() {
    echo -e "${BLUE}[DEBUG]${NC} $1"
}

# Check prerequisites
check_prerequisites() {
    log_info "Checking prerequisites..."
    
    # Check Docker
    if ! command -v docker &> /dev/null; then
        log_error "Docker is not installed. Please install Docker first."
        exit 1
    fi
    
    # Check docker compose command
    if ! docker compose version &> /dev/null; then
        log_error "docker compose is not available. Please ensure Docker with compose plugin is installed."
        exit 1
    fi
    
    # Check if we're in the right directory
    if [ ! -f "config/open5gs/docker-compose.yml" ]; then
        log_error "Cannot find config/open5gs/docker-compose.yml. Please run from project root."
        exit 1
    fi
    
    log_info "Prerequisites check passed"
}

# Start Open5GS
start_open5gs() {
    log_info "Starting Open5GS 5G Core..."
    cd config/open5gs
    
    # Stop any existing Open5GS containers
    docker compose down 2>/dev/null || true
    
    # Start Open5GS
    docker compose up -d
    
    # Wait for services to be ready
    log_info "Waiting for Open5GS services to initialize..."
    sleep 10
    
    # Check if AMF is running
    if docker compose ps | grep -q "open5gs_amf.*Up"; then
        log_info "Open5GS AMF is running"
    else
        log_error "Open5GS AMF failed to start"
        docker compose logs amf
        return 1
    fi
    
    cd ../..
    return 0
}

# Stop Open5GS
stop_open5gs() {
    log_info "Stopping Open5GS..."
    cd config/open5gs
    docker compose down
    cd ../..
}

# Start gNodeB
start_gnodeb() {
    if [ "$USE_OUR_GNB" = true ]; then
        log_info "Starting Albor Space GNodeB..."
        
        # Check if our Docker container is running
        if docker ps --format '{{.Names}}' | grep -q "^albor-gnb-dev$"; then
            log_info "Using existing Albor GNodeB container"
            docker exec -d albor-gnb-dev /workspace/target/release/albor_gnodeb > logs/albor_gnb.log 2>&1
        else
            log_error "Albor GNodeB container not running. Please start it first."
            return 1
        fi
    else
        log_info "Starting srsRAN Project gNodeB..."
        
        # Check if srsRAN Project gNodeB is available
        if ! command -v gnb &> /dev/null; then
            log_error "srsRAN Project gNodeB not found. Please install it first."
            return 1
        fi
        
        # Create log directory
        mkdir -p logs
        
        # Start gNodeB with our configuration
        gnb -c config/srsran_gnb/gnb_zmq.yml > logs/srsran_gnb.log 2>&1 &
        GNB_PID=$!
        
        # Save PID for later cleanup
        echo $GNB_PID > logs/gnb.pid
    fi
    
    # Wait for gNodeB to initialize
    sleep 5
    
    # Check if gNodeB connected to AMF
    if grep -q "N2: Connection to AMF" logs/*gnb.log 2>/dev/null; then
        log_info "gNodeB successfully connected to AMF"
    else
        log_warn "gNodeB may not have connected to AMF yet"
    fi
    
    return 0
}

# Start UE
start_ue() {
    log_info "Starting srsUE in 5G SA mode..."
    
    # Check if srsUE is available
    if ! command -v srsue &> /dev/null; then
        log_error "srsUE not found. Please install srsRAN 4G first."
        return 1
    fi
    
    # Create network namespace for UE (optional but recommended)
    sudo ip netns add ue1 2>/dev/null || true
    
    # Start UE with our configuration
    sudo ip netns exec ue1 srsue config/srsue/ue_nr_zmq.conf > logs/srsue.log 2>&1 &
    UE_PID=$!
    
    # Save PID for later cleanup
    echo $UE_PID > logs/ue.pid
    
    # Wait for UE to start
    sleep 5
    
    return 0
}

# Monitor test
monitor_test() {
    log_info "Monitoring 5G SA connection for 30 seconds..."
    
    # Create a monitoring script to check status
    for i in {1..30}; do
        echo -ne "\rMonitoring... $i/30 seconds"
        
        # Check if UE is connected
        if grep -q "RRC Connected" logs/srsue.log 2>/dev/null; then
            echo -e "\n${GREEN}✓ UE reached RRC Connected state!${NC}"
        fi
        
        if grep -q "PDU Session Establishment successful" logs/srsue.log 2>/dev/null; then
            echo -e "${GREEN}✓ PDU Session established successfully!${NC}"
            if grep -q "IP:" logs/srsue.log 2>/dev/null; then
                IP=$(grep "IP:" logs/srsue.log | tail -1)
                echo -e "${GREEN}✓ UE assigned $IP${NC}"
            fi
            break
        fi
        
        sleep 1
    done
    echo ""
}

# Stop all components
stop_all() {
    log_info "Stopping all components..."
    
    # Stop UE
    if [ -f logs/ue.pid ]; then
        UE_PID=$(cat logs/ue.pid)
        sudo kill $UE_PID 2>/dev/null || true
        rm logs/ue.pid
    fi
    
    # Remove network namespace
    sudo ip netns del ue1 2>/dev/null || true
    
    # Stop gNodeB
    if [ -f logs/gnb.pid ]; then
        GNB_PID=$(cat logs/gnb.pid)
        kill $GNB_PID 2>/dev/null || true
        rm logs/gnb.pid
    fi
    
    # Stop Open5GS
    stop_open5gs
}

# Generate report
generate_report() {
    log_info "Generating test report..."
    
    REPORT_FILE="logs/5g_sa_test_report_$(date +%Y%m%d_%H%M%S).txt"
    
    {
        echo "=== 5G SA Test Report ==="
        echo "Date: $(date)"
        echo ""
        
        echo "=== Configuration ==="
        echo "- Core: Open5GS"
        if [ "$USE_OUR_GNB" = true ]; then
            echo "- gNodeB: Albor Space GNodeB"
        else
            echo "- gNodeB: srsRAN Project"
        fi
        echo "- UE: srsUE (5G SA mode)"
        echo "- Interface: ZeroMQ"
        echo ""
        
        echo "=== Test Results ==="
        
        # Check AMF connection
        if docker compose -f config/open5gs/docker-compose.yml ps | grep -q "open5gs_amf.*Up"; then
            echo "✓ AMF: Running"
        else
            echo "✗ AMF: Not running"
        fi
        
        # Check gNodeB-AMF connection
        if grep -q "N2: Connection to AMF" logs/*gnb.log 2>/dev/null; then
            echo "✓ gNodeB-AMF: Connected"
        else
            echo "✗ gNodeB-AMF: Not connected"
        fi
        
        # Check UE cell search
        if grep -q "Found Cell" logs/srsue.log 2>/dev/null; then
            echo "✓ Cell Search: Success"
        else
            echo "✗ Cell Search: Failed"
        fi
        
        # Check RRC connection
        if grep -q "RRC Connected" logs/srsue.log 2>/dev/null; then
            echo "✓ RRC Connection: Established"
        else
            echo "✗ RRC Connection: Failed"
        fi
        
        # Check registration
        if grep -q "Registration complete" logs/srsue.log 2>/dev/null; then
            echo "✓ 5G Registration: Complete"
        else
            echo "✗ 5G Registration: Failed"
        fi
        
        # Check PDU session
        if grep -q "PDU Session Establishment successful" logs/srsue.log 2>/dev/null; then
            echo "✓ PDU Session: Established"
            if grep -q "IP:" logs/srsue.log 2>/dev/null; then
                IP=$(grep "IP:" logs/srsue.log | tail -1)
                echo "  $IP"
            fi
        else
            echo "✗ PDU Session: Failed"
        fi
        
        echo ""
        echo "=== Log Files ==="
        echo "- Open5GS AMF: docker compose -f config/open5gs/docker-compose.yml logs amf"
        echo "- gNodeB: logs/*gnb.log"
        echo "- UE: logs/srsue.log"
        
    } | tee "$REPORT_FILE"
    
    log_info "Report saved to: $REPORT_FILE"
}

# Main test sequence
main() {
    log_info "Starting 5G SA Complete Setup Test"
    
    # Check prerequisites
    check_prerequisites
    
    # Create log directory
    mkdir -p logs
    
    # Trap to ensure cleanup on exit
    trap stop_all EXIT
    
    # Start Open5GS
    if ! start_open5gs; then
        log_error "Failed to start Open5GS"
        exit 1
    fi
    
    # Start gNodeB
    if ! start_gnodeb; then
        log_error "Failed to start gNodeB"
        exit 1
    fi
    
    # Start UE
    if ! start_ue; then
        log_error "Failed to start UE"
        exit 1
    fi
    
    # Monitor the test
    monitor_test
    
    # Generate report
    generate_report
    
    log_info "Test completed. Check the report for details."
}

# Run main function
main