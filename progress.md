# Albor 5G gNodeB - Progress Report

## Current Status

### ✅ Implemented
- **PHY Layer**: Complete implementation including:
  - OFDM modulation/demodulation with proper FFT/IFFT
  - PSS/SSS generation and placement
  - PBCH encoding with Polar codes and DMRS
  - Resource grid mapping
  - Frame/slot/symbol timing
  - Resampler (15.36 MHz → 11.52 MHz)
  
- **MAC Layer**: Basic scheduler for SSB and SIB1 transmission

- **RRC Layer**: MIB and SIB1 message generation

- **ZMQ Interface**: Bidirectional RF interface compatible with srsUE

### ❌ Not Working
- **Cell Detection**: UE cannot detect the cell - PHY sends mostly zeros (only SSB symbols have data)
- **Signal Transmission**: Non-SSB symbols are transmitted as zeros, creating ~90% zero signal
- **RRC Connection**: Not reached due to cell detection failure

## Technical Configuration
- Band: 3 (1842.5 MHz)
- Bandwidth: 10 MHz
- Subcarrier Spacing: 15 kHz
- PCI: 1
- Sample Rate: 11.52 MHz (via resampling from 15.36 MHz)
- ZMQ Ports: TX 2000, RX 2001

## Known Issues
1. **Primary Blocker**: PHY transmits zeros for non-SSB symbols (>90% of the signal)
2. **srsRAN Baseline**: Achieved 4/6 criteria (PRACH detection fails over ZMQ)
3. **Architecture Issue**: Resource grid only populated for SSB, not for all symbols

## Next Steps
1. **Fix Zero Transmission**: Implement proper symbol population for all OFDM symbols
2. **Add Reference Signals**: Populate non-SSB symbols with cell-specific reference signals
3. **Continuous Transmission**: Ensure all symbols have appropriate 5G NR signals
4. **Test Cell Detection**: Verify UE can detect properly populated signal

## Test Commands
```bash
# Test Albor implementation
./test_albor.sh

# Test srsRAN baseline (once working)
./test_srsran.sh
```

## Key Files
- Main entry: `gnb/src/main.rs`
- PHY implementation: `layers/src/phy/`
- Configuration: `config/albor_gnb/gnb_albor.yml`
- Test logs: `logs/` (timestamped directories)