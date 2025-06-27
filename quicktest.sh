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

# Albor Space 5G GNodeB Quick Test Script
# This script orchestrates testing by calling the appropriate test script
# MUST follow test methodology: ONLY test_srsran.sh and test_albor.sh allowed
#
# Usage: ./quicktest.sh [--srsran]
#   --srsran: Use srsRAN reference gNodeB for baseline testing
#   (default): Use Albor gNodeB implementation
#
# CRITICAL: This script ONLY calls the two allowed test scripts

set -e  # Exit on error

# Parse command line arguments
USE_SRSRAN=false
while [[ $# -gt 0 ]]; do
    case $1 in
        --srsran)
            USE_SRSRAN=true
            shift
            ;;
        *)
            echo "Unknown option: $1"
            echo "Usage: $0 [--srsran]"
            echo "  --srsran: Test with srsRAN reference gNodeB"
            echo "  (default): Test with Albor gNodeB implementation"
            exit 1
            ;;
    esac
done

# Color codes for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Logging functions
log_info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Check if we're running inside Docker container
if [ -f /.dockerenv ]; then
    log_info "Running inside Docker container (good!)"
    
    # Create dated log directory
    LOG_DIR="logs/$(date +%Y%m%d_%H%M%S)"
    mkdir -p "$LOG_DIR"
    log_info "Created log directory: $LOG_DIR"
    
    # Execute the appropriate test script
    if [ "$USE_SRSRAN" = true ]; then
        log_info "Executing test_srsran.sh for reference baseline..."
        ./test_srsran.sh 2>&1 | tee "$LOG_DIR/test_output.log"
        exit_code=${PIPESTATUS[0]}
    else
        log_info "Executing test_albor.sh for our implementation..."
        ./test_albor.sh 2>&1 | tee "$LOG_DIR/test_output.log"
        exit_code=${PIPESTATUS[0]}
    fi
    
    # Report results
    if [ $exit_code -eq 0 ]; then
        log_info "Test completed successfully. Logs in $LOG_DIR"
    else
        log_error "Test failed. Check logs in $LOG_DIR"
    fi
    
    exit $exit_code
    
else
    # We're NOT inside Docker container - need to check if container is running
    log_info "Not running inside Docker container. Checking for running container..."
    
    CONTAINER_NAME="albor-gnb-dev"
    
    # Check if container is already running
    if docker ps --format '{{.Names}}' | grep -q "^${CONTAINER_NAME}$"; then
        log_info "Container '$CONTAINER_NAME' is already running. Using docker exec..."
        # Execute this script inside the running container
        # Check if we have a TTY
        if [ -t 0 ]; then
            docker exec -it "$CONTAINER_NAME" /workspace/quicktest.sh "$@"
        else
            docker exec "$CONTAINER_NAME" /workspace/quicktest.sh "$@"
        fi
        exit $?
    else
        log_error "Container '$CONTAINER_NAME' is not running."
        log_error "Please start the container first with:"
        log_error "  docker run -it --name $CONTAINER_NAME -v \$(pwd):/workspace albor-gnb-dev:latest bash"
        log_error "Then run quicktest.sh from inside the container."
        exit 1
    fi
fi