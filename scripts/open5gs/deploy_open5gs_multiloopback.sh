#!/bin/bash
# Complete deployment script for Open5GS with multi-loopback configuration
# This script runs all necessary steps to deploy Open5GS without port conflicts

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

echo "=== Open5GS Multi-Loopback Deployment Script ==="
echo "This script will:"
echo "1. Set up loopback interfaces (127.0.0.2-12)"
echo "2. Update Open5GS configurations to use unique IPs"
echo "3. Start Open5GS core with proper isolation"
echo ""

# Function to run a script with error handling
run_script() {
    local script_name=$1
    local script_path="${SCRIPT_DIR}/${script_name}"
    
    echo ""
    echo ">>> Running: $script_name"
    echo "================================================"
    
    if [ ! -x "$script_path" ]; then
        echo "Error: $script_path not found or not executable"
        return 1
    fi
    
    if "$script_path"; then
        echo "✓ $script_name completed successfully"
        return 0
    else
        echo "✗ $script_name failed"
        return 1
    fi
}

# Main execution
main() {
    # Check if running with proper privileges
    if [ "$EUID" -ne 0 ] && ! sudo -n true 2>/dev/null; then
        echo "This script requires sudo privileges."
        echo "Please run with: sudo $0"
        exit 1
    fi
    
    # Step 1: Set up loopback interfaces
    if ! run_script "setup_loopback_interfaces.sh"; then
        echo "Failed to set up loopback interfaces. Aborting."
        exit 1
    fi
    
    # Step 2: Update Open5GS configurations
    if ! run_script "update_open5gs_configs.sh"; then
        echo "Failed to update configurations. Aborting."
        exit 1
    fi
    
    # Step 3: Start Open5GS core
    if ! run_script "start_open5gs_core.sh"; then
        echo "Failed to start Open5GS. Check the logs for details."
        exit 1
    fi
    
    echo ""
    echo "=== Deployment Complete ==="
    echo ""
    echo "Quick verification commands:"
    echo "  - Check interfaces: ip addr show | grep '127.0.0.'"
    echo "  - Check services: ps aux | grep open5gs"
    echo "  - Check AMF SCTP: netstat -tuln | grep 38412"
    echo "  - Check logs: tail -f /var/log/open5gs/*.log"
    echo ""
    echo "To add a test subscriber (srsUE default):"
    echo "  mongosh --host 127.0.0.2 open5gs --eval 'db.subscribers.insertOne({imsi: \"001010000000001\", ...})'"
    echo ""
    echo "For gNodeB connection:"
    echo "  - AMF address: 127.0.0.4:38412"
    echo "  - PLMN: 00101"
    echo "  - TAC: 7"
    echo ""
}

# Handle Ctrl+C gracefully
trap 'echo ""; echo "Deployment interrupted. You may need to clean up manually."; exit 1' INT

# Run main function
main "$@"