#!/bin/bash
# test_comparison.sh - Quick comparison test between srsRAN and Albor
# Tests cell detection only, without AMF

set -e

CONTAINER_NAME="albor-gnb-dev"
LOG_DIR="./logs/$(date +%Y%m%d_%H%M%S)_comparison"
mkdir -p "$LOG_DIR"

echo "=== gNodeB Comparison Test ==="
echo "Log directory: $LOG_DIR"

# Cleanup function
cleanup() {
    docker exec $CONTAINER_NAME bash -c "pkill -9 gnb albor_gnodeb srsue 2>/dev/null || true"
    docker exec $CONTAINER_NAME bash -c "rm -f /tmp/ue_stdin 2>/dev/null || true"
    docker exec $CONTAINER_NAME bash -c "pkill -9 -f 'sleep infinity' 2>/dev/null || true"
    sleep 1
}

# Test function
test_gnodeb() {
    local test_name=$1
    local gnb_cmd=$2
    local log_prefix=$3
    
    echo ""
    echo "--- Testing $test_name ---"
    cleanup
    
    # Start gNodeB
    echo "Starting $test_name..."
    docker exec -d $CONTAINER_NAME bash -c "$gnb_cmd > /tmp/${log_prefix}_gnb.log 2>&1"
    sleep 3
    
    # Check if running
    if docker exec $CONTAINER_NAME pgrep -f "$log_prefix" > /dev/null; then
        echo "✓ $test_name is running"
    else
        echo "✗ $test_name failed to start"
        docker exec $CONTAINER_NAME tail -20 /tmp/${log_prefix}_gnb.log 2>/dev/null
        return 1
    fi
    
    # Start UE
    echo "Starting srsUE..."
    docker exec $CONTAINER_NAME bash -c "mkfifo /tmp/ue_stdin 2>/dev/null || true"
    docker exec -d $CONTAINER_NAME bash -c "sleep infinity > /tmp/ue_stdin"
    docker exec -d $CONTAINER_NAME bash -c "export LD_LIBRARY_PATH=/opt/srsran/lib:\$LD_LIBRARY_PATH && /opt/srsran/bin/srsue /workspace/config/srsue/ue_nr_zmq_10mhz.conf --rat.nr.dl_nr_arfcn 368500 --rat.nr.ssb_nr_arfcn 368410 --rat.nr.nof_prb 52 --rat.nr.scs 15 --rat.nr.ssb_scs 15 < /tmp/ue_stdin > /tmp/${log_prefix}_ue.log 2>&1"
    
    # Monitor for cell detection
    local found=false
    for i in {1..10}; do
        if docker exec $CONTAINER_NAME grep -q "Found Cell" /tmp/${log_prefix}_ue.log 2>/dev/null; then
            found=true
            echo "✓ Cell detected after $i seconds!"
            break
        fi
        sleep 1
    done
    
    if [ "$found" = false ]; then
        echo "✗ No cell detected after 10 seconds"
    fi
    
    # Copy logs
    docker exec $CONTAINER_NAME cat /tmp/${log_prefix}_gnb.log > "$LOG_DIR/${log_prefix}_gnb.log" 2>/dev/null
    docker exec $CONTAINER_NAME cat /tmp/${log_prefix}_ue.log > "$LOG_DIR/${log_prefix}_ue.log" 2>/dev/null
    
    return 0
}

# Test 1: Albor gNodeB
test_gnodeb "Albor gNodeB" "/workspace/target/release/albor_gnodeb -c /workspace/config/albor_gnb/gnb_albor.yml" "albor"

# Test 2: srsRAN gNodeB with command line (no AMF)
test_gnodeb "srsRAN gNodeB" "cd /opt/srsran_project && /opt/srsran_project/bin/gnb --gnb_id 1 cell_cfg --dl_arfcn 368500 --band 3 --channel_bandwidth_MHz 10 cell_cfg --common_scs 15kHz --plmn 00101 --tac 7 --pci 1 ru_sdr --device_driver zmq --srate 11.52 --tx_gain 75 --rx_gain 75 ru_sdr --device_args 'tx_port=tcp://127.0.0.1:2000,rx_port=tcp://127.0.0.1:2001,base_srate=11.52e6' log --filename /tmp/srsran_gnb_internal.log --all_level info" "gnb"

# Summary
echo ""
echo "=========================================="
echo "TEST SUMMARY:"
echo "=========================================="
echo "Albor logs: $LOG_DIR/albor_*.log"
echo "srsRAN logs: $LOG_DIR/gnb_*.log"
echo ""
echo "Key differences to check:"
echo "1. SSB transmission patterns"
echo "2. MIB content"
echo "3. System Information scheduling"
echo "=========================================="

cleanup
echo "Test complete."