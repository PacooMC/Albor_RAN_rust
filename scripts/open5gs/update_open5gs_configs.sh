#!/bin/bash
# Update Open5GS configuration files to use unique loopback addresses
# This prevents port conflicts by assigning each component its own IP

set -e

CONFIG_DIR="/opt/open5gs/etc/open5gs"
BACKUP_DIR="${CONFIG_DIR}/backups/$(date +%Y%m%d_%H%M%S)"

echo "=== Updating Open5GS configurations for multi-loopback deployment ==="

# Check if configuration directory exists
if [ ! -d "$CONFIG_DIR" ]; then
    echo "Error: Open5GS configuration directory not found at $CONFIG_DIR"
    echo "Please ensure Open5GS is installed."
    exit 1
fi

# Create backup directory
echo "Creating backup directory: $BACKUP_DIR"
sudo mkdir -p "$BACKUP_DIR"

# Function to update YAML configuration with proper component addresses
update_component_config() {
    local component=$1
    local ip_addr=$2
    local file="${CONFIG_DIR}/${component}.yaml"
    
    if [ ! -f "$file" ]; then
        echo "  ! Warning: $file not found, skipping..."
        return
    fi
    
    echo "Updating $component configuration..."
    
    # Create backup
    sudo cp "$file" "${BACKUP_DIR}/${component}.yaml"
    
    # Create temporary file for modifications
    local temp_file="/tmp/${component}_config_$$.yaml"
    sudo cp "$file" "$temp_file"
    
    # Update component's own address
    case $component in
        nrf)
            # NRF listens on its own address
            sudo sed -i "s/addr: *127\.0\.0\.[0-9]\+/addr: $ip_addr/g" "$temp_file"
            sudo sed -i "s/addr: *0\.0\.0\.0/addr: $ip_addr/g" "$temp_file"
            
            # Update PLMN for 00101
            sudo sed -i '/serving:/,/mnc:/ s/mcc: *[0-9]\+/mcc: 001/' "$temp_file"
            sudo sed -i '/serving:/,/mnc:/ s/mnc: *[0-9]\+/mnc: 01/' "$temp_file"
            ;;
            
        amf)
            # AMF specific configurations
            # Update SBI (HTTP) interface
            sudo sed -i "/sbi:/,/addr:/ s/addr:.*/addr: $ip_addr/" "$temp_file"
            
            # Update NGAP (SCTP) interface - critical for gNodeB connection
            sudo sed -i "/ngap:/,/dev:/ {
                /addr:/ s/addr:.*/addr: $ip_addr/
            }" "$temp_file"
            
            # Update metrics interface
            sudo sed -i "/metrics:/,/addr:/ s/addr:.*/addr: $ip_addr/" "$temp_file"
            
            # Update PLMN to 00101
            sudo sed -i '/plmn_id:/,/mnc:/ s/mcc: *[0-9]\+/mcc: 001/' "$temp_file"
            sudo sed -i '/plmn_id:/,/mnc:/ s/mnc: *[0-9]\+/mnc: 01/' "$temp_file"
            
            # Update TAC to 7
            sudo sed -i 's/tac: *[0-9]\+/tac: 7/g' "$temp_file"
            
            # Update PLMN support list
            sudo sed -i '/plmn_support:/,/tac:/ {
                /mcc:/ s/mcc: *[0-9]\+/mcc: 001/
                /mnc:/ s/mnc: *[0-9]\+/mnc: 01/
                /tac:/ s/tac: *[0-9]\+/tac: 7/
            }' "$temp_file"
            
            # Update NRF reference to correct IP
            sudo sed -i "/nrf:/,/}/ s/addr: *127\.0\.0\.[0-9]\+/addr: 127.0.0.3/g" "$temp_file"
            ;;
            
        smf)
            # SMF configurations
            sudo sed -i "/sbi:/,/addr:/ s/addr:.*/addr: $ip_addr/" "$temp_file"
            sudo sed -i "/metrics:/,/addr:/ s/addr:.*/addr: $ip_addr/" "$temp_file"
            
            # Update PFCP interface to bind to SMF's address
            sudo sed -i "/pfcp:/,/addr:/ s/addr:.*/addr: $ip_addr/" "$temp_file"
            
            # Update NRF reference
            sudo sed -i "/nrf:/,/}/ s/addr: *127\.0\.0\.[0-9]\+/addr: 127.0.0.3/g" "$temp_file"
            
            # Ensure subnet configuration remains 10.45.0.0/16
            # This is typically already correct in default configs
            ;;
            
        upf)
            # UPF configurations - special handling for GTP and PFCP
            sudo sed -i "/pfcp:/,/addr:/ s/addr:.*/addr: $ip_addr/" "$temp_file"
            
            # Update GTP-U interface
            sudo sed -i "/gtpu:/,/advertise_name:/ {
                /addr:/ s/addr:.*/addr: $ip_addr/
            }" "$temp_file"
            
            # Update metrics
            sudo sed -i "/metrics:/,/addr:/ s/addr:.*/addr: $ip_addr/" "$temp_file"
            
            # Update SMF reference for PFCP association
            sudo sed -i "/smf:/,/}/ s/addr: *127\.0\.0\.[0-9]\+/addr: 127.0.0.5/g" "$temp_file"
            ;;
            
        ausf|udm|udr|pcf|nssf|bsf)
            # Standard NF configurations
            sudo sed -i "/sbi:/,/addr:/ s/addr:.*/addr: $ip_addr/" "$temp_file"
            sudo sed -i "/metrics:/,/addr:/ s/addr:.*/addr: $ip_addr/" "$temp_file"
            
            # Update NRF reference
            sudo sed -i "/nrf:/,/}/ s/addr: *127\.0\.0\.[0-9]\+/addr: 127.0.0.3/g" "$temp_file"
            ;;
    esac
    
    # Update references to other components based on our IP mapping
    # MongoDB
    sudo sed -i "s/mongodb:\/\/127\.0\.0\.[0-9]\+/mongodb:\/\/127.0.0.2/g" "$temp_file"
    sudo sed -i "s/mongodb:\/\/localhost/mongodb:\/\/127.0.0.2/g" "$temp_file"
    
    # Copy updated config back
    sudo cp "$temp_file" "$file"
    rm -f "$temp_file"
    
    echo "  ✓ Updated $component to use $ip_addr"
}

# Update MongoDB configuration if it exists
update_mongodb_config() {
    local mongo_conf="/etc/mongod.conf"
    if [ -f "$mongo_conf" ]; then
        echo "Updating MongoDB configuration..."
        sudo cp "$mongo_conf" "${BACKUP_DIR}/mongod.conf"
        
        # Update bind IP - MongoDB should listen on 127.0.0.2
        sudo sed -i 's/bindIp: *127\.0\.0\.1/bindIp: 127.0.0.2,127.0.0.1/' "$mongo_conf"
        
        # Ensure it's not binding to all interfaces
        sudo sed -i 's/bindIp: *0\.0\.0\.0/bindIp: 127.0.0.2,127.0.0.1/' "$mongo_conf"
        
        echo "  ✓ Updated MongoDB to bind to 127.0.0.2"
    fi
}

# Component to IP mapping
declare -A COMPONENT_IPS=(
    ["nrf"]="127.0.0.3"
    ["amf"]="127.0.0.4"
    ["smf"]="127.0.0.5"
    ["pcf"]="127.0.0.6"
    ["udr"]="127.0.0.7"
    ["udm"]="127.0.0.8"
    ["ausf"]="127.0.0.9"
    ["upf"]="127.0.0.10"
    ["nssf"]="127.0.0.11"
    ["bsf"]="127.0.0.12"
)

# Update each component
for component in nrf amf smf upf ausf udm udr pcf nssf bsf; do
    if [ -n "${COMPONENT_IPS[$component]}" ]; then
        update_component_config "$component" "${COMPONENT_IPS[$component]}"
    fi
done

# Update MongoDB
update_mongodb_config

# Create a summary configuration file
cat > /tmp/open5gs_network_map.txt << EOF
Open5GS Network Configuration Map
=================================

Component   IP Address      Ports
---------   -----------     -----
MongoDB     127.0.0.2       27017
NRF         127.0.0.3       7777 (SBI)
AMF         127.0.0.4       38412 (NGAP/SCTP), 7777 (SBI)
SMF         127.0.0.5       7777 (SBI), 8805 (PFCP)
PCF         127.0.0.6       7777 (SBI)
UDR         127.0.0.7       7777 (SBI)
UDM         127.0.0.8       7777 (SBI)
AUSF        127.0.0.9       7777 (SBI)
UPF         127.0.0.10      2152 (GTP-U), 8805 (PFCP)
NSSF        127.0.0.11      7777 (SBI)
BSF         127.0.0.12      7777 (SBI)

Network Settings:
- PLMN: 00101 (MCC=001, MNC=01)
- TAC: 7
- DN Subnet: 10.45.0.0/16
- IPv6 Subnet: 2001:db8:cafe::/48

gNodeB Connection:
- Connect to AMF at: 127.0.0.4:38412 (SCTP)
- Use GTP-U endpoint: 127.0.0.10:2152
EOF

sudo cp /tmp/open5gs_network_map.txt "${CONFIG_DIR}/network_map.txt"

echo ""
echo "=== Configuration update complete ==="
echo "Backup created at: $BACKUP_DIR"
echo "Network map saved to: ${CONFIG_DIR}/network_map.txt"
echo ""
echo "All Open5GS components have been configured to use unique loopback addresses."
echo "This prevents port conflicts and allows all components to run simultaneously."
echo ""
echo "Next step: Run ./start_open5gs_core.sh to start Open5GS with the new configuration"
echo ""