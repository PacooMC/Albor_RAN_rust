#!/bin/bash
# Extended test for Albor gNodeB cell detection with srsUE

LOG_DIR="logs/$(date +%Y%m%d_%H%M%S)_extended_test"
mkdir -p "$LOG_DIR"

echo "[INFO] Starting Albor gNodeB (no AMF connection)..."
docker exec albor-gnb-dev bash -c "cd /workspace && ./target/debug/albor_gnodeb --config config/albor_gnb/gnb_albor.yml" > "$LOG_DIR/gnb.log" 2>&1 &
GNB_PID=$!

echo "[INFO] Waiting 5 seconds for gNodeB initialization..."
sleep 5

echo "[INFO] Starting srsUE..."
docker exec albor-gnb-dev bash -c "export LD_LIBRARY_PATH=/opt/srsran/lib:\$LD_LIBRARY_PATH && /opt/srsran/bin/srsue /workspace/config/srsue/ue_nr_zmq_10mhz.conf" > "$LOG_DIR/ue.log" 2>&1 &
UE_PID=$!

echo "[INFO] Running for 30 seconds to allow cell detection..."
for i in {1..30}; do
    echo -n "."
    # Check if UE found cell
    if grep -q "Found Cell" "$LOG_DIR/ue.log" 2>/dev/null; then
        echo
        echo "[SUCCESS] UE found cell!"
        break
    fi
    sleep 1
done
echo

echo "[INFO] Checking for cell detection..."
if grep -q "Found Cell\|RRC Connected" "$LOG_DIR/ue.log" 2>/dev/null; then
    echo "[SUCCESS] Cell detection successful!"
    grep -E "Found Cell|RRC Connected" "$LOG_DIR/ue.log"
else
    echo "[WARNING] No cell detection found in UE logs"
fi

echo "[INFO] Checking for PRACH detection..."
if grep -q "PRACH occasion detected" "$LOG_DIR/gnb.log" 2>/dev/null; then
    echo "[SUCCESS] PRACH detected by gNodeB!"
    grep "PRACH occasion detected" "$LOG_DIR/gnb.log" | tail -5
fi

echo "[INFO] Stopping processes..."
docker exec albor-gnb-dev bash -c "pkill -f albor_gnodeb"
docker exec albor-gnb-dev bash -c "pkill -f srsue"

echo
echo "=== Final Status ==="
echo "GNodeB log: $LOG_DIR/gnb.log"
echo "UE log: $LOG_DIR/ue.log"
echo
echo "=== UE Status ==="
grep -E "Attaching|Search|Found|Cell|band|PRACH|RRC|Connected" "$LOG_DIR/ue.log" | tail -10
echo
echo "=== ZMQ Communication Status ==="
echo "TX requests from UE: $(grep -c "TX request from UE" "$LOG_DIR/gnb.log")"
echo "Samples received: $(grep -c "samples from UE" "$LOG_DIR/gnb.log")"