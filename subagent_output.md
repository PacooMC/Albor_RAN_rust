# Cell Detection Debug Session Summary

## Current Status
The UE cannot detect the Albor gNodeB cell despite all components appearing to work correctly.

## What's Working
1. **ZMQ Interface**: Successfully established bidirectional communication
   - gNodeB binds to TX port 2000, RX port 2001
   - UE connects and sends/receives samples
   - No ZMQ errors

2. **SSB Transmission**: All components transmitted correctly
   - PSS: Generated with correct m-sequence, amplitude 2.0 (6 dB)
   - SSS: Generated with correct sequences
   - PBCH: Encoded with MIB data
   - DMRS: Added to PBCH symbols 1 and 3 (v=1 for cell_id=1)

3. **Timing**: SSB transmitted every 20ms as expected
   - Frame structure correct
   - Symbol timing accurate

4. **Signal Power**: Improved from -15.93 dB to -6.93 dB
   - IQ samples have reasonable magnitudes (~0.4)
   - Baseband gain adjusted for stronger signal

## Critical Finding
**The reference srsRAN gNodeB also fails with the same configuration**, suggesting the issue is not specific to our implementation.

## Root Cause Analysis
The most likely issue is **SSB frequency placement**. In 5G NR:
- The SSB is not necessarily at the carrier center frequency
- There's a concept of "pointA" (reference frequency)
- SSB has an offset from pointA (k_SSB parameter)
- Our implementation places SSB centered at DC (0 Hz offset)

## Missing Implementation
1. **SSB Subcarrier Offset (k_SSB)**:
   - Not configured in YAML
   - Not implemented in our code
   - Critical for UE to find the SSB

2. **Point A Calculation**:
   - Need to calculate the correct frequency offset
   - SSB should be placed at specific GSCN frequencies

## Recommended Fix
1. Add SSB offset calculation based on:
   - ARFCN (368500)
   - Band (3)
   - Bandwidth (10 MHz)

2. Implement k_SSB parameter:
   - Read from config if provided
   - Calculate automatically if not
   - Apply offset when mapping SSB to resource grid

3. Update resource grid mapping:
   ```rust
   // Instead of centering at DC:
   let ssb_start_sc = -(240 / 2) as i16;
   
   // Apply k_SSB offset:
   let ssb_start_sc = -(240 / 2) as i16 + k_ssb;
   ```

## Test Results
- Signal is being transmitted
- ZMQ communication working
- But UE cannot find cell at expected frequency
- This points to frequency/offset issue

## Next Steps
1. Research correct SSB placement for Band 3, ARFCN 368500
2. Implement k_SSB parameter support
3. Test with corrected frequency offset