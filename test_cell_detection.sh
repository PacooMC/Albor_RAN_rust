#!/bin/bash
# Test Albor gNodeB cell detection with srsUE (no AMF)

LOG_DIR="logs/$(date +%Y%m%d_%H%M%S)_cell_test"
mkdir -p "$LOG_DIR"

echo "[INFO] Starting Albor gNodeB (no AMF connection)..."
docker exec albor-gnb-dev bash -c "cd /workspace && ./target/release/albor_gnodeb --config config/albor_gnb/gnb_albor.yml" > "$LOG_DIR/gnb.log" 2>&1 &
GNB_PID=$!

echo "[INFO] Waiting 3 seconds for gNodeB initialization..."
sleep 3

echo "[INFO] Starting srsUE..."
docker exec albor-gnb-dev bash -c "export LD_LIBRARY_PATH=/opt/srsran/lib:\$LD_LIBRARY_PATH && /opt/srsran/bin/srsue /workspace/config/srsue/ue_nr_zmq_10mhz.conf" > "$LOG_DIR/ue.log" 2>&1 &
UE_PID=$!

echo "[INFO] Running for 10 seconds..."
for i in {1..10}; do
    echo -n "."
    sleep 1
done
echo

echo "[INFO] Stopping processes..."
docker exec albor-gnb-dev bash -c "pkill -f albor_gnodeb"
docker exec albor-gnb-dev bash -c "pkill -f srsue"

echo "[INFO] Test completed. Checking results..."
echo
echo "=== GNodeB Log Summary ==="
tail -20 "$LOG_DIR/gnb.log"
echo
echo "=== UE Log Summary ==="
grep -E "Found Cell|cell_search|Searching|PSS|SSS|PRACH|Random Access|RRC" "$LOG_DIR/ue.log" | tail -20
echo
echo "[INFO] Full logs available in: $LOG_DIR"