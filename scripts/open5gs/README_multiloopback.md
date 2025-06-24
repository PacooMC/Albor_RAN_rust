# Open5GS Multi-Loopback Configuration

This directory contains scripts to deploy Open5GS 5G Core with multiple loopback interfaces to avoid port conflicts.

## Problem Solved

When all Open5GS components bind to 127.0.0.1, they compete for the same ports (especially port 7777 for SBI interfaces). This causes startup failures and makes debugging difficult.

## Solution

Each component gets its own loopback IP address:

| Component | IP Address   | Ports                          |
|-----------|-------------|--------------------------------|
| MongoDB   | 127.0.0.2   | 27017                         |
| NRF       | 127.0.0.3   | 7777 (SBI)                    |
| AMF       | 127.0.0.4   | 38412 (NGAP/SCTP), 7777 (SBI)|
| SMF       | 127.0.0.5   | 7777 (SBI), 8805 (PFCP)       |
| PCF       | 127.0.0.6   | 7777 (SBI)                    |
| UDR       | 127.0.0.7   | 7777 (SBI)                    |
| UDM       | 127.0.0.8   | 7777 (SBI)                    |
| AUSF      | 127.0.0.9   | 7777 (SBI)                    |
| UPF       | 127.0.0.10  | 2152 (GTP-U), 8805 (PFCP)     |
| NSSF      | 127.0.0.11  | 7777 (SBI)                    |
| BSF       | 127.0.0.12  | 7777 (SBI)                    |

## Scripts

### 1. setup_loopback_interfaces.sh
Creates loopback interfaces 127.0.0.2 through 127.0.0.12. Safe to run multiple times.

### 2. update_open5gs_configs.sh
Updates all Open5GS configuration files to use the assigned IP addresses. Creates backups before modifying.

### 3. start_open5gs_core.sh
Enhanced startup script that:
- Checks prerequisites
- Sets up loopback interfaces if needed
- Starts MongoDB on 127.0.0.2
- Starts all Open5GS components in correct order
- Verifies components are listening on expected ports

### 4. deploy_open5gs_multiloopback.sh
Convenience script that runs all three scripts in sequence for complete deployment.

## Usage

### Quick Deployment
```bash
# Deploy everything at once
sudo ./scripts/open5gs/deploy_open5gs_multiloopback.sh
```

### Step by Step
```bash
# 1. Set up network interfaces
sudo ./scripts/open5gs/setup_loopback_interfaces.sh

# 2. Update configurations
sudo ./scripts/open5gs/update_open5gs_configs.sh

# 3. Start Open5GS
sudo ./scripts/open5gs/start_open5gs_core.sh
```

### With quicktest.sh
```bash
# Use multi-loopback deployment with testing
./quicktest.sh --multiloopback

# This automatically:
# - Deploys Open5GS with multi-loopback
# - Configures gNodeB to connect to AMF at 127.0.0.4:38412
# - Runs the complete test
```

## Docker Considerations

When running in Docker, SCTP support requires either:
- Running with `--privileged` flag
- Adding capabilities: `--cap-add=NET_ADMIN,SYS_ADMIN`

The scripts detect and warn about SCTP issues automatically.

## Troubleshooting

### Check Interfaces
```bash
ip addr show | grep '127.0.0.'
```

### Check Services
```bash
ps aux | grep open5gs
netstat -tuln | grep -E '(7777|38412|2152)'
```

### Check Logs
```bash
tail -f /var/log/open5gs/*.log
```

### Verify AMF SCTP
```bash
# Should show SCTP LISTEN on 127.0.0.4:38412
ss -tuln | grep 38412
```

## Network Configuration

- PLMN: 00101 (MCC=001, MNC=01)
- TAC: 7
- DN Subnet: 10.45.0.0/16
- IPv6 Subnet: 2001:db8:cafe::/48

## gNodeB Connection

Configure your gNodeB to connect to:
- AMF N2 Interface: 127.0.0.4:38412 (SCTP)
- GTP-U Endpoint: 127.0.0.10:2152