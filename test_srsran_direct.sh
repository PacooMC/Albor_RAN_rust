#!/bin/bash
# test_srsran_direct.sh - Direct test of srsRAN gNodeB + srsUE without Open5GS
# Uses YAML config file to avoid command line parsing issues

set -e

# Configuration
CONTAINER_NAME="albor-gnb-dev"
LOG_DIR="./logs/$(date +%Y%m%d_%H%M%S)_srsran_direct"

# Create log directory on host
mkdir -p "$LOG_DIR"

echo "[$(date +%H:%M:%S)] Starting srsRAN direct test (no AMF)"
echo "[$(date +%H:%M:%S)] Log directory: $LOG_DIR"

# Step 1: Clean up
echo "[$(date +%H:%M:%S)] Cleaning up previous runs..."
docker exec $CONTAINER_NAME bash -c "pkill -9 gnb 2>/dev/null || true"
docker exec $CONTAINER_NAME bash -c "pkill -9 srsue 2>/dev/null || true"
docker exec $CONTAINER_NAME bash -c "rm -f /tmp/ue_stdin 2>/dev/null || true"
docker exec $CONTAINER_NAME bash -c "pkill -9 -f 'sleep infinity' 2>/dev/null || true"
sleep 1

# Step 2: Create a minimal gNodeB config without AMF
echo "[$(date +%H:%M:%S)] Creating minimal gNodeB config..."
docker exec $CONTAINER_NAME bash -c "cat > /tmp/gnb_minimal.yml << 'EOF'
# Minimal srsRAN gNodeB config - No AMF
# Based on sacred 10MHz configuration

# gNodeB configuration
gnb_id: 1
gnb_id_bit_length: 22
gtp_bind_addr: 127.0.0.11
gtp_advertise_addr: 127.0.0.11

# No AMF configuration - running standalone

# Logging
log:
  filename: /tmp/gnb.log
  all_level: info

# Cell configuration - 10 MHz on Band 3
cell_cfg:
  dl_arfcn: 368500
  band: 3
  channel_bandwidth_MHz: 10
  common_scs: 15
  plmn: '00101'
  tac: 7
  pci: 1
  prach:
    prach_config_index: 1
    prach_root_sequence_index: 1
    zero_correlation_zone: 0
    prach_frequency_start: 1

# 5G SA cell paging configuration
paging:
  search_space0_index: 0
  default_paging_cycle: 128
  nof_pf: 1
  paging_drx: rf32
  usim_key: '00112233445566778899AABBCCDDEEFF'
  usim_opc: '63BFA50EE6523365FF14C1F45F88737D'
  usim_k_encr_algo: 0
  usim_k_int_algo: 2

# PDCCH configuration
pdcch:
  common:
    coreset0_index: 6
    ss0_index: 0
    ss1_index: 0
    ra_search_space_index: 1
  dedicated:
    coreset1_rb_start: 0
    coreset1_l_crb: 48
    coreset1_duration: 1
    dci_format_0_1_and_1_1: false

# PDSCH configuration
pdsch:
  mcs_table: qam256
  min_ue_mcs: 0
  max_ue_mcs: 28
  rar_mcs_index: 4

# PRACH configuration
prach:
  prach_config_index: 1
  prach_root_sequence_index: 1
  zero_correlation_zone: 0
  prach_frequency_start: 1

# PUSCH configuration
pusch:
  mcs_table: qam256
  min_ue_mcs: 0
  max_ue_mcs: 28
  p0_nominal_with_grant: -90
  msg3_delta_preamble: 6
  msg3_delta_prach: 0
  expected_ack_nack_feedback_time_in_us: 0

# PUCCH configuration
pucch:
  sr_resource_config:
  - sr_resource_id: 1
    periodicity: 40
    offset: 8
    resource_mapping:
      one_port_nof_rb: 1
      preamble_length: 16
      start_symb: 14
      nof_symb: 2
  f1_params:
    occ_supported: 1
    nof_cyclic_shifts: 12
    interslot_freq_hop: 1
  f2_params:
    nof_prb: 1
    simultaneous_harq_ack_csi: 1
    max_code_rate: 0.35

# TDD configuration (5ms periodicity)
tdd_ul_dl_cfg:
  dl_ul_tx_period: 5
  nof_dl_slots: 7
  nof_dl_symbols: 6
  nof_ul_slots: 2
  nof_ul_symbols: 4

# Scheduler expert configuration
scheduler_expert_cfg:
  si_window_length: 35
  ra_response_window: 10
  enable_pdcch_precoding: 1
  enable_sib_mbsfn: 0
  nr_pdsch_mcs_table: 2
  enable_ul_64qam: 1

# Radio Unit configuration - ZMQ
ru_sdr:
  device_driver: zmq
  device_args: tx_port=tcp://127.0.0.1:2000,rx_port=tcp://127.0.0.1:2001,base_srate=11.52e6
  srate: 11.52
  tx_gain: 75
  rx_gain: 75

# Test mode and additional configurations
test_mode:
  test_ue:
    rnti: 0x44
    imsi: '001010000000001'
    nof_ues: 1

# MAC-NR PCAP
pcap:
  mac_enable: true
  mac_filename: /tmp/gnb_mac.pcap
  ngap_enable: false  # No AMF connection
EOF"

# Step 3: Start srsRAN gNodeB
echo "[$(date +%H:%M:%S)] Starting srsRAN gNodeB..."
docker exec -d $CONTAINER_NAME bash -c "cd /opt/srsran_project && /opt/srsran_project/bin/gnb -c /tmp/gnb_minimal.yml > /tmp/gnb_stdout.log 2>&1"

# Wait for gNodeB to initialize
echo "[$(date +%H:%M:%S)] Waiting 5 seconds for gNodeB initialization..."
sleep 5

# Check if gNodeB started
echo "[$(date +%H:%M:%S)] Checking gNodeB status..."
if docker exec $CONTAINER_NAME pgrep -f gnb > /dev/null; then
    echo "[$(date +%H:%M:%S)] ✓ gNodeB process is running"
else
    echo "[$(date +%H:%M:%S)] ✗ gNodeB failed to start"
    docker exec $CONTAINER_NAME cat /tmp/gnb_stdout.log
    exit 1
fi

# Step 4: Start srsUE
echo "[$(date +%H:%M:%S)] Starting srsUE..."
docker exec $CONTAINER_NAME bash -c "mkfifo /tmp/ue_stdin 2>/dev/null || true"
docker exec -d $CONTAINER_NAME bash -c "sleep infinity > /tmp/ue_stdin"
docker exec -d $CONTAINER_NAME bash -c "export LD_LIBRARY_PATH=/opt/srsran/lib:\$LD_LIBRARY_PATH && /opt/srsran/bin/srsue /workspace/config/srsue/ue_nr_zmq_10mhz.conf --rat.nr.dl_nr_arfcn 368500 --rat.nr.ssb_nr_arfcn 368410 --rat.nr.nof_prb 52 --rat.nr.scs 15 --rat.nr.ssb_scs 15 < /tmp/ue_stdin > /tmp/ue.log 2>&1"

# Step 5: Monitor for 15 seconds
echo "[$(date +%H:%M:%S)] Monitoring for 15 seconds..."
for i in {1..15}; do
    echo -n "[$(date +%H:%M:%S)] Test running... ($i/15) "
    
    # Check for cell detection
    if docker exec $CONTAINER_NAME grep -q "Found Cell" /tmp/ue.log 2>/dev/null; then
        echo "✓ CELL DETECTED!"
        break
    else
        echo ""
    fi
    
    sleep 1
done

# Step 6: Copy logs to host
echo "[$(date +%H:%M:%S)] Copying logs to host..."
docker exec $CONTAINER_NAME cat /tmp/gnb.log 2>/dev/null > "$LOG_DIR/gnb.log" || echo "No gnb.log"
docker exec $CONTAINER_NAME cat /tmp/gnb_stdout.log 2>/dev/null > "$LOG_DIR/gnb_stdout.log" || echo "No gnb_stdout.log"
docker exec $CONTAINER_NAME cat /tmp/ue.log 2>/dev/null > "$LOG_DIR/ue.log" || echo "No ue.log"
docker exec $CONTAINER_NAME cat /tmp/gnb_minimal.yml > "$LOG_DIR/gnb_minimal.yml"

# Step 7: Show results
echo ""
echo "=========================================="
echo "TEST RESULTS:"
echo "=========================================="

# Check for cell detection
if docker exec $CONTAINER_NAME grep -q "Found Cell" /tmp/ue.log 2>/dev/null; then
    echo "✅ SUCCESS: srsUE detected srsRAN gNodeB cell!"
    echo ""
    echo "Cell detection details:"
    docker exec $CONTAINER_NAME grep -A5 -B5 "Found Cell" /tmp/ue.log 2>/dev/null | tail -20
else
    echo "❌ FAILED: srsUE did not detect cell"
    echo ""
    echo "UE log tail:"
    docker exec $CONTAINER_NAME tail -30 /tmp/ue.log 2>/dev/null || echo "No UE log"
    echo ""
    echo "gNodeB log tail:"
    docker exec $CONTAINER_NAME tail -30 /tmp/gnb_stdout.log 2>/dev/null || echo "No gNodeB log"
fi

echo "=========================================="

# Cleanup
echo "[$(date +%H:%M:%S)] Cleaning up..."
docker exec $CONTAINER_NAME bash -c "pkill -9 gnb 2>/dev/null || true"
docker exec $CONTAINER_NAME bash -c "pkill -9 srsue 2>/dev/null || true"
docker exec $CONTAINER_NAME bash -c "rm -f /tmp/ue_stdin 2>/dev/null || true"
docker exec $CONTAINER_NAME bash -c "pkill -9 -f 'sleep infinity' 2>/dev/null || true"

echo "[$(date +%H:%M:%S)] Test complete. Logs saved to: $LOG_DIR"