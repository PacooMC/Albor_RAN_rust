#!/bin/bash
# Configure Open5GS for localhost operation with Albor RAN settings

set -e

CONFIG_DIR="/opt/open5gs/etc/open5gs"

echo "=== Configuring Open5GS for localhost operation ==="

# Function to update YAML configuration
update_config() {
    local file=$1
    local component=$2
    
    if [ -f "$file" ]; then
        echo "Updating $component configuration..."
        
        # Create backup
        sudo cp "$file" "${file}.backup" 2>/dev/null || true
        
        # Update to use localhost addresses
        sudo sed -i 's/127\.0\.0\.[0-9]\+/127.0.0.1/g' "$file"
        
        # For AMF, set specific NGAP address and PLMN/TAC
        if [[ "$component" == "amf" ]]; then
            # Update NGAP address
            sudo sed -i '/ngap:/,/addr:/ s/addr:.*/addr: 127.0.0.1/' "$file"
            
            # Update PLMN to 00101
            sudo sed -i '/plmn_id:/,/mnc:/ s/mcc:.*/mcc: 001/' "$file"
            sudo sed -i '/plmn_id:/,/mnc:/ s/mnc:.*/mnc: 01/' "$file"
            
            # Update TAC to 7
            sudo sed -i '/tac:/ s/tac:.*/tac: 7/' "$file"
            
            # Update PLMN support list
            sudo sed -i '/plmn_support:/,/mnc:/ s/mcc:.*/mcc: 001/' "$file"
            sudo sed -i '/plmn_support:/,/mnc:/ s/mnc:.*/mnc: 01/' "$file"
        fi
        
        # For NRF, update PLMN
        if [[ "$component" == "nrf" ]]; then
            sudo sed -i '/serving:/,/mnc:/ s/mcc:.*/mcc: 001/' "$file"
            sudo sed -i '/serving:/,/mnc:/ s/mnc:.*/mnc: 01/' "$file"
        fi
        
        # For SMF/UPF, ensure correct subnet configuration
        if [[ "$component" == "smf" ]] || [[ "$component" == "upf" ]]; then
            # Already configured for 10.45.0.0/16 by default
            echo "  - Subnet already configured as 10.45.0.0/16"
        fi
        
        echo "  - Updated $component configuration"
    else
        echo "Warning: $file not found"
    fi
}

# Update all component configurations
for component in nrf amf smf upf ausf udm udr pcf nssf bsf; do
    update_config "${CONFIG_DIR}/${component}.yaml" "$component"
done

# Create a simple subscriber add script
cat > /tmp/add-subscriber.sh << 'EOF'
#!/bin/bash
# Add a test subscriber to Open5GS

echo "Adding test subscriber..."

# Default values matching srsUE configuration
IMSI=${1:-"001010000000001"}
KEY=${2:-"00112233445566778899aabbccddeeff"}
OPC=${3:-"63bfa50ee6523365ff14c1f45f88737d"}

# Create subscriber document
mongosh open5gs --eval "
db.subscribers.insertOne({
    imsi: '$IMSI',
    subscribed_rau_tau_timer: 12,
    network_access_mode: 0,
    subscriber_status: 0,
    access_restriction_data: 32,
    slice: [{
        sst: 1,
        default_indicator: true,
        session: [{
            name: 'internet',
            type: 3,
            pcc_rule: [],
            ambr: {
                uplink: { value: 1, unit: 3 },
                downlink: { value: 1, unit: 3 }
            },
            qos: {
                index: 9,
                arp: {
                    priority_level: 8,
                    pre_emption_capability: 1,
                    pre_emption_vulnerability: 1
                }
            }
        }]
    }],
    ambr: {
        uplink: { value: 1, unit: 3 },
        downlink: { value: 1, unit: 3 }
    },
    security: {
        k: '$KEY',
        amf: '8000',
        op: null,
        opc: '$OPC'
    },
    schema_version: 1,
    __v: 0
});
"

echo "Subscriber $IMSI added successfully"
EOF

chmod +x /tmp/add-subscriber.sh
sudo mv /tmp/add-subscriber.sh "${CONFIG_DIR}/../bin/add-subscriber.sh"

echo ""
echo "=== Configuration Complete ==="
echo "Open5GS is now configured for localhost operation with:"
echo "  - PLMN: 00101 (MCC=001, MNC=01)"
echo "  - TAC: 7"
echo "  - AMF NGAP: 127.0.0.1:38412"
echo "  - All components on localhost"
echo ""
echo "To add a test subscriber:"
echo "  ${CONFIG_DIR}/../bin/add-subscriber.sh [IMSI] [KEY] [OPC]"
echo ""