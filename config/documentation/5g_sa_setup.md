# 5G SA (Standalone) Complete Setup Documentation

## Overview

This document provides a comprehensive guide for setting up a complete 5G SA network using:
- **Open5GS**: 5G Core Network (AMF, SMF, UPF, etc.)
- **srsRAN Project**: 5G gNodeB
- **srsRAN 4G**: UE in NR mode
- **ZeroMQ**: Virtual RF interface for testing without hardware

## Architecture

```
┌─────────────┐     NGAP      ┌──────────────┐     N2/N3      ┌─────────────┐
│   srsUE     │ ◄───────────► │ srsRAN gNB   │ ◄────────────► │  Open5GS    │
│  (NR mode)  │      ZMQ       │              │      SCTP      │  5G Core    │
└─────────────┘                └──────────────┘                └─────────────┘
```

## Complete Configuration Parameters

### 1. Network Parameters

#### PLMN Configuration
- **MCC**: 001 (Test network)
- **MNC**: 01 (Test network)
- **PLMN**: "00101"

#### Network Slicing
- **SST**: 1 (eMBB - Enhanced Mobile Broadband)
- **SD**: 1 (Slice Differentiator)

#### Tracking Area
- **TAC**: 7 (Tracking Area Code)

### 2. Frequency and Bandwidth Configuration

#### Band 3 FDD Configuration (Recommended for srsUE)
- **Band**: 3
- **DL ARFCN**: 368500
- **DL Frequency**: 1842.5 MHz
- **UL Frequency**: 1747.5 MHz
- **Bandwidth**: 10 MHz (recommended) or 20 MHz
- **Subcarrier Spacing**: 15 kHz
- **Sample Rate**: 23.04 MHz

#### Alternative Band 78 TDD Configuration
- **Band**: 78
- **DL ARFCN**: 632628
- **Center Frequency**: 3489.42 MHz
- **Bandwidth**: 20 MHz
- **Subcarrier Spacing**: 30 kHz
- **Sample Rate**: 30.72 MHz

### 3. Cell Configuration

#### Physical Cell Configuration
- **PCI**: 1 (Physical Cell Identity)
- **Cell ID**: 0x19B (Global Cell ID)

#### SSB Configuration
- **SSB Period**: 20 ms
- **SSB Position Bitmap**: "10000000" (for 15 kHz SCS)
- **SSB Subcarrier Offset**: 6

#### CORESET#0 Configuration
- **CORESET0 Index**: 12 (for 10 MHz bandwidth)
- **Search Space 0 Index**: 0

### 4. PRACH Configuration
- **PRACH Config Index**: 1 (FDD)
- **PRACH Frequency Start**: 1
- **Zero Correlation Zone**: 12
- **Preamble Format**: 0
- **Target Power**: -110 dBm
- **Max Preamble Attempts**: 7
- **Power Ramping Step**: 4 dB
- **RA Response Window**: 10 slots

### 5. PDSCH/PUSCH Configuration
- **MCS Table**: qam64 (64-QAM modulation)
- **Max Layers**: 1 (SISO)
- **DMRS Type A Position**: 2
- **DMRS Additional Position**: 0

### 6. ZMQ Interface Configuration

#### gNodeB ZMQ Settings
- **TX Port**: tcp://127.0.0.1:2000
- **RX Port**: tcp://127.0.0.1:2001
- **Base Sample Rate**: 23.04e6
- **ID**: gnb

#### UE ZMQ Settings
- **TX Port**: tcp://127.0.0.1:2001
- **RX Port**: tcp://127.0.0.1:2000
- **Base Sample Rate**: 23.04e6
- **ID**: ue

### 7. RF Configuration
- **TX Gain**: 75 dB (ZMQ)
- **RX Gain**: 75 dB (ZMQ)
- **Frequency Offset**: 0 Hz

### 8. Core Network Configuration

#### AMF Configuration
- **AMF Address**: 10.53.1.2
- **AMF Port**: 38412
- **Bind Address**: 10.53.1.1

#### UPF Configuration
- **UPF Address**: 10.53.1.7
- **GTPU Port**: 2152
- **PFCP Port**: 8805

#### Network Configuration
- **DN Network**: 10.45.0.0/16
- **DNS Servers**: 8.8.8.8, 8.8.4.4

### 9. UE Configuration

#### USIM Parameters
- **IMSI**: 001010000000001
- **Key (K)**: 00112233445566778899aabbccddeeff
- **OPc**: 63BFA50EE6523365FF14C1F45F88737D
- **Algorithm**: milenage

#### APN Configuration
- **APN**: internet
- **PDU Session Type**: IPv4
- **Network Interface**: tun_srsue

### 10. Timing and Protocol Configuration

#### RRC Timers
- **T300**: 1000 ms
- **T301**: 1000 ms
- **T310**: 1000 ms
- **T311**: 10000 ms
- **N310**: 1
- **N311**: 1

#### MAC Configuration
- **SR Periodicity**: 20 ms
- **BSR Timer**: 20 ms
- **PHR Timer**: 50 ms

### 11. PDCCH Configuration (Critical for srsUE)
- **Dedicated Search Space Type**: common
- **DCI Format 0_1 and 1_1**: false (use fallback formats)
- **Aggregation Level**: {1, 2, 4, 8, 16}
- **PDCCH Candidates**: Based on search space configuration

### 12. System Information Configuration

#### SIB1 Scheduling
- **Periodicity**: 160 ms
- **SI Window Length**: 20 ms
- **RMSI CORESET**: Same as CORESET#0

#### MIB Parameters
- **SFN**: System Frame Number (0-1023)
- **Half Frame Bit**: 0 or 1
- **SSB Subcarrier Offset**: 6
- **DMRS Type A Position**: 2
- **PDCCH Config SIB1**: From CORESET#0 tables
- **Cell Barred**: Not barred
- **Intra Frequency Reselection**: Allowed

## Key Technical Considerations

### 1. Sample Rate Calculation
- For 15 kHz SCS: Sample rate = 1.92 MHz * 12 = 23.04 MHz
- For 30 kHz SCS: Sample rate = 1.92 MHz * 16 = 30.72 MHz

### 2. Resource Grid
- 10 MHz @ 15 kHz = 624 subcarriers (52 RBs)
- 20 MHz @ 15 kHz = 1272 subcarriers (106 RBs)

### 3. OFDM Symbols
- Normal CP: 14 symbols per slot
- Slot duration @ 15 kHz: 1 ms
- Slot duration @ 30 kHz: 0.5 ms

### 4. Frame Structure
- Radio frame: 10 ms
- Subframes: 10 x 1 ms
- Slots per frame @ 15 kHz: 10
- Slots per frame @ 30 kHz: 20

## Validation Checklist

1. **Core Network**: AMF accessible at configured address
2. **gNodeB**: Successfully connects to AMF via NGAP
3. **RF Interface**: ZMQ ports bound without conflicts
4. **Cell Search**: UE detects PSS/SSS
5. **MIB Decode**: UE successfully decodes PBCH
6. **SIB1 Reception**: UE receives and decodes SIB1
7. **PRACH**: UE sends preamble, gNB detects
8. **RRC Connection**: Complete RRC setup
9. **Registration**: UE registers with 5G Core
10. **PDU Session**: Data plane established

## Common Issues and Solutions

### Issue 1: UE Cannot Find Cell
- Check frequency configuration matches
- Verify PSS/SSS generation
- Ensure adequate signal power (TX gain)
- Confirm sample rate configuration

### Issue 2: SIB1 Decode Failure
- Verify CORESET#0 configuration
- Check PDCCH/PDSCH encoding
- Ensure DMRS mapping is correct
- Validate SI-RNTI scrambling

### Issue 3: PRACH Not Detected
- Confirm PRACH configuration index
- Check PRACH occasion timing
- Verify root sequence configuration
- Ensure power levels adequate

### Issue 4: Registration Failure
- Verify PLMN matches Core config
- Check TAC configuration
- Ensure NGAP connection active
- Validate USIM credentials

## Performance Optimization

### CPU Requirements
- Minimum: 2 cores for basic setup
- Recommended: 4+ cores for stable operation
- Real-time kernel beneficial

### Memory Requirements
- Minimum: 4 GB RAM
- Recommended: 8+ GB RAM

### Network Optimization
- Disable firewall for testing
- Enable IP forwarding
- Configure proper MTU size (1500)
- Use network namespaces for isolation

## Testing Commands

### Start Order
1. Start Open5GS Core
2. Start srsRAN gNodeB
3. Start srsUE

### Monitoring
- gNodeB logs: Check for "Cell pci=1, bw=10 MHz"
- AMF logs: Look for "NGAP connection established"
- UE logs: Monitor for "RRC Connected" and "PDU Session Established"

This configuration represents a complete, validated setup for 5G SA operation with srsRAN and Open5GS.