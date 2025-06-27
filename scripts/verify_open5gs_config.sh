#!/bin/bash
# verify_open5gs_config.sh - Verify and fix Open5GS configuration

set -e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

log_info() { echo -e "${GREEN}[INFO]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }
log_warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }

log_info "=== Verifying Open5GS Configuration ==="

CONFIG_DIR="config/open5gs_native/config"

# Expected IP mappings
declare -A EXPECTED_IPS=(
    ["mongodb"]="127.0.0.2"
    ["nrf"]="127.0.0.3"
    ["amf"]="127.0.0.4"
    ["smf"]="127.0.0.5"
    ["ausf"]="127.0.0.6"
    ["udm"]="127.0.0.7"
    ["pcf"]="127.0.0.8"
    ["udr"]="127.0.0.9"
    ["upf"]="127.0.0.10"
    ["gnb"]="127.0.0.11"
)

log_info "Checking configuration files..."

# Check each component configuration
ALL_CORRECT=true

# Check NRF
NRF_IP=$(grep -A1 "server:" "$CONFIG_DIR/nrf.yaml" | grep "address:" | awk '{print $3}')
if [ "$NRF_IP" = "${EXPECTED_IPS[nrf]}" ]; then
    log_info "✓ NRF configured correctly: $NRF_IP"
else
    log_error "✗ NRF has wrong IP: $NRF_IP (expected: ${EXPECTED_IPS[nrf]})"
    ALL_CORRECT=false
fi

# Check AMF
AMF_SBI_IP=$(grep -A1 "sbi:" "$CONFIG_DIR/amf.yaml" -A2 | grep "address:" | head -1 | awk '{print $3}')
AMF_NGAP_IP=$(grep -A1 "ngap:" "$CONFIG_DIR/amf.yaml" -A2 | grep "address:" | head -1 | awk '{print $3}')
if [ "$AMF_SBI_IP" = "${EXPECTED_IPS[amf]}" ] && [ "$AMF_NGAP_IP" = "${EXPECTED_IPS[amf]}" ]; then
    log_info "✓ AMF configured correctly: $AMF_SBI_IP"
else
    log_error "✗ AMF has wrong IP: SBI=$AMF_SBI_IP, NGAP=$AMF_NGAP_IP (expected: ${EXPECTED_IPS[amf]})"
    ALL_CORRECT=false
fi

# Check SMF
SMF_IP=$(grep -A1 "server:" "$CONFIG_DIR/smf.yaml" | grep "address:" | awk '{print $3}')
if [ "$SMF_IP" = "${EXPECTED_IPS[smf]}" ]; then
    log_info "✓ SMF configured correctly: $SMF_IP"
else
    log_error "✗ SMF has wrong IP: $SMF_IP (expected: ${EXPECTED_IPS[smf]})"
    ALL_CORRECT=false
fi

# Check AUSF
AUSF_IP=$(grep -A1 "server:" "$CONFIG_DIR/ausf.yaml" | grep "address:" | awk '{print $3}')
if [ "$AUSF_IP" = "${EXPECTED_IPS[ausf]}" ]; then
    log_info "✓ AUSF configured correctly: $AUSF_IP"
else
    log_error "✗ AUSF has wrong IP: $AUSF_IP (expected: ${EXPECTED_IPS[ausf]})"
    ALL_CORRECT=false
fi

# Check UDM
UDM_IP=$(grep -A1 "server:" "$CONFIG_DIR/udm.yaml" | grep "address:" | awk '{print $3}')
if [ "$UDM_IP" = "${EXPECTED_IPS[udm]}" ]; then
    log_info "✓ UDM configured correctly: $UDM_IP"
else
    log_error "✗ UDM has wrong IP: $UDM_IP (expected: ${EXPECTED_IPS[udm]})"
    ALL_CORRECT=false
fi

# Check PCF
PCF_IP=$(grep -A1 "server:" "$CONFIG_DIR/pcf.yaml" | grep "address:" | awk '{print $3}')
if [ "$PCF_IP" = "${EXPECTED_IPS[pcf]}" ]; then
    log_info "✓ PCF configured correctly: $PCF_IP"
else
    log_error "✗ PCF has wrong IP: $PCF_IP (expected: ${EXPECTED_IPS[pcf]})"
    ALL_CORRECT=false
fi

# Check UDR
UDR_IP=$(grep -A1 "server:" "$CONFIG_DIR/udr.yaml" | grep "address:" | awk '{print $3}')
if [ "$UDR_IP" = "${EXPECTED_IPS[udr]}" ]; then
    log_info "✓ UDR configured correctly: $UDR_IP"
else
    log_error "✗ UDR has wrong IP: $UDR_IP (expected: ${EXPECTED_IPS[udr]})"
    ALL_CORRECT=false
fi

# Check UPF
UPF_PFCP_IP=$(grep -A1 "pfcp:" "$CONFIG_DIR/upf.yaml" -A2 | grep "address:" | head -1 | awk '{print $3}')
UPF_GTPU_IP=$(grep -A1 "gtpu:" "$CONFIG_DIR/upf.yaml" -A2 | grep "address:" | head -1 | awk '{print $3}')
if [ "$UPF_PFCP_IP" = "${EXPECTED_IPS[upf]}" ] && [ "$UPF_GTPU_IP" = "${EXPECTED_IPS[upf]}" ]; then
    log_info "✓ UPF configured correctly: $UPF_PFCP_IP"
else
    log_error "✗ UPF has wrong IP: PFCP=$UPF_PFCP_IP, GTPU=$UPF_GTPU_IP (expected: ${EXPECTED_IPS[upf]})"
    ALL_CORRECT=false
fi

# Check MongoDB URI in all configs
log_info ""
log_info "Checking MongoDB URIs..."
for config in "$CONFIG_DIR"/*.yaml; do
    if grep -q "db_uri:" "$config" 2>/dev/null; then
        MONGO_URI=$(grep "db_uri:" "$config" | awk '{print $2}')
        COMPONENT=$(basename "$config" .yaml)
        if [ "$MONGO_URI" = "mongodb://${EXPECTED_IPS[mongodb]}/open5gs" ]; then
            log_info "✓ $COMPONENT: MongoDB URI correct"
        else
            log_error "✗ $COMPONENT: Wrong MongoDB URI: $MONGO_URI"
            ALL_CORRECT=false
        fi
    fi
done

# Check if NRF is correctly referenced in all component configs
log_info ""
log_info "Checking NRF references..."
for config in "$CONFIG_DIR"/*.yaml; do
    if grep -q "nrf:" "$config" 2>/dev/null && [ "$(basename $config)" != "nrf.yaml" ]; then
        NRF_URI=$(grep -A1 "nrf:" "$config" | grep "uri:" | awk '{print $3}')
        COMPONENT=$(basename "$config" .yaml)
        if [ "$NRF_URI" = "http://${EXPECTED_IPS[nrf]}:7777" ]; then
            log_info "✓ $COMPONENT: NRF URI correct"
        else
            log_error "✗ $COMPONENT: Wrong NRF URI: $NRF_URI"
            ALL_CORRECT=false
        fi
    fi
done

echo ""
if [ "$ALL_CORRECT" = "true" ]; then
    log_info "✅ All configurations are correct!"
else
    log_error "❌ Some configurations need fixing!"
fi

# Check if running processes are on correct IPs
log_info ""
log_info "Checking running processes..."

check_process_binding() {
    local process=$1
    local expected_ip=$2
    local port=$3
    
    if pgrep -f "$process" > /dev/null; then
        if netstat -tuln 2>/dev/null | grep -q "$expected_ip:$port"; then
            log_info "✓ $process is running on $expected_ip:$port"
        else
            log_warn "⚠ $process is running but not on $expected_ip:$port"
            netstat -tuln 2>/dev/null | grep ":$port" || echo "  Port $port not found"
        fi
    else
        log_warn "⚠ $process is not running"
    fi
}

# Check running services
check_process_binding "mongod" "${EXPECTED_IPS[mongodb]}" "27017"
check_process_binding "open5gs-nrfd" "${EXPECTED_IPS[nrf]}" "7777"
check_process_binding "open5gs-amfd" "${EXPECTED_IPS[amf]}" "38412"
check_process_binding "open5gs-smfd" "${EXPECTED_IPS[smf]}" "7777"
check_process_binding "open5gs-upfd" "${EXPECTED_IPS[upf]}" "2152"

log_info ""
log_info "=== Configuration verification complete ==="