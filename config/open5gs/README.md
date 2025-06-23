# Open5GS 5G Core Network Configuration

This directory contains the Docker Compose setup for Open5GS 5G Core Network configured for Albor Space 5G GNodeB project.

## Network Configuration

- **PLMN**: 00101 (MCC=001, MNC=01)
- **TAC**: 7
- **AMF N2 Interface**: Port 38412 (SCTP)
- **WebUI**: Port 9999

## Quick Start

1. Start the 5G Core:
```bash
cd config/open5gs
docker-compose up -d
```

2. Access the WebUI:
- URL: http://localhost:9999
- Default credentials: admin/1423

3. Add UE subscribers through the WebUI:
- IMSI: 001010000000001
- Key: 465B5CE8B199B49FAA5F0A2EE238A6BC
- OPc: E8ED289DEBA952E4283B54E88E6183CA

## Network Architecture

The deployment includes all 5G Core Network Functions:
- **AMF** (Access and Mobility Management Function) - 10.1.1.18
- **SMF** (Session Management Function) - 10.1.1.19
- **UPF** (User Plane Function) - 10.1.1.20
- **NRF** (Network Repository Function) - 10.1.1.10
- **AUSF** (Authentication Server Function) - 10.1.1.12
- **UDM** (Unified Data Management) - 10.1.1.13
- **UDR** (Unified Data Repository) - 10.1.1.14
- **PCF** (Policy Control Function) - 10.1.1.15
- **BSF** (Binding Support Function) - 10.1.1.16
- **NSSF** (Network Slice Selection Function) - 10.1.1.17
- **SCP** (Service Communication Proxy) - 10.1.1.11

## Integration with Albor GNodeB

The AMF exposes port 38412 for the N2 interface. Configure your GNodeB to connect to:
- AMF Address: <host-ip>:38412
- PLMN: 00101
- TAC: 7

## Configuration Files

Additional configuration files need to be created for each network function. The AMF configuration (`config/amf.yaml`) is provided as a reference with the correct PLMN and TAC settings.