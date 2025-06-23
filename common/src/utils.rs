//! Common Utilities
//! 
//! Provides utility functions used across the GNodeB implementation

use bytes::{Bytes, BytesMut, BufMut};
use tracing::trace;

/// Convert a byte slice to hex string for debugging
pub fn bytes_to_hex(data: &[u8]) -> String {
    data.iter()
        .map(|b| format!("{:02x}", b))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Calculate CRC-24 for 5G NR
pub fn crc24(data: &[u8]) -> u32 {
    const CRC24_POLY: u32 = 0x1864CFB;
    let mut crc: u32 = 0;
    
    for byte in data {
        crc ^= (*byte as u32) << 16;
        for _ in 0..8 {
            if crc & 0x800000 != 0 {
                crc = (crc << 1) ^ CRC24_POLY;
            } else {
                crc <<= 1;
            }
        }
    }
    
    crc & 0xFFFFFF
}

/// Calculate CRC-16 for 5G NR
pub fn crc16(data: &[u8]) -> u16 {
    const CRC16_POLY: u16 = 0x1021;
    let mut crc: u16 = 0;
    
    for byte in data {
        crc ^= (*byte as u16) << 8;
        for _ in 0..8 {
            if crc & 0x8000 != 0 {
                crc = (crc << 1) ^ CRC16_POLY;
            } else {
                crc <<= 1;
            }
        }
    }
    
    crc
}

/// Pack bits into bytes (MSB first)
pub fn pack_bits(bits: &[bool]) -> Bytes {
    let mut bytes = BytesMut::with_capacity((bits.len() + 7) / 8);
    
    for chunk in bits.chunks(8) {
        let mut byte = 0u8;
        for (i, &bit) in chunk.iter().enumerate() {
            if bit {
                byte |= 1 << (7 - i);
            }
        }
        bytes.put_u8(byte);
    }
    
    bytes.freeze()
}

/// Unpack bytes into bits (MSB first)
pub fn unpack_bits(bytes: &[u8]) -> Vec<bool> {
    let mut bits = Vec::with_capacity(bytes.len() * 8);
    
    for &byte in bytes {
        for i in 0..8 {
            bits.push((byte & (1 << (7 - i))) != 0);
        }
    }
    
    bits
}

/// Round up to next power of 2
pub fn next_power_of_2(n: u32) -> u32 {
    if n == 0 {
        return 1;
    }
    
    let mut v = n;
    v -= 1;
    v |= v >> 1;
    v |= v >> 2;
    v |= v >> 4;
    v |= v >> 8;
    v |= v >> 16;
    v + 1
}

/// Calculate resource blocks from bandwidth and subcarrier spacing
pub fn calculate_nrb(bandwidth_hz: u32, scs_khz: u16) -> u16 {
    // Each RB has 12 subcarriers
    const SUBCARRIERS_PER_RB: u32 = 12;
    
    let scs_hz = scs_khz as u32 * 1000;
    let total_subcarriers = bandwidth_hz / scs_hz;
    let nrb = total_subcarriers / SUBCARRIERS_PER_RB;
    
    trace!("Calculated {} RBs for {}Hz bandwidth with {}kHz SCS", 
           nrb, bandwidth_hz, scs_khz);
    
    nrb as u16
}

/// Time utilities for slot/frame calculations
pub mod time {
    /// Slot duration in microseconds for different SCS
    pub fn slot_duration_us(scs_khz: u16) -> u32 {
        match scs_khz {
            15 => 1000,    // 1 ms
            30 => 500,     // 0.5 ms
            60 => 250,     // 0.25 ms
            120 => 125,    // 0.125 ms
            240 => 62,     // 0.0625 ms (approximated)
            _ => panic!("Invalid SCS: {}", scs_khz),
        }
    }
    
    /// Number of slots per frame (10ms)
    pub fn slots_per_frame(scs_khz: u16) -> u16 {
        match scs_khz {
            15 => 10,
            30 => 20,
            60 => 40,
            120 => 80,
            240 => 160,
            _ => panic!("Invalid SCS: {}", scs_khz),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_bytes_to_hex() {
        let data = vec![0x12, 0x34, 0xAB, 0xCD];
        assert_eq!(bytes_to_hex(&data), "12 34 ab cd");
    }
    
    #[test]
    fn test_crc24() {
        let data = b"Hello";
        let crc = crc24(data);
        assert_eq!(crc & 0xFFFFFF, crc); // Ensure 24-bit result
    }
    
    #[test]
    fn test_bit_packing() {
        let bits = vec![true, false, true, false, true, false, true, false];
        let packed = pack_bits(&bits);
        assert_eq!(packed[0], 0xAA); // 10101010
        
        let unpacked = unpack_bits(&packed);
        assert_eq!(unpacked[..8], bits);
    }
    
    #[test]
    fn test_next_power_of_2() {
        assert_eq!(next_power_of_2(0), 1);
        assert_eq!(next_power_of_2(1), 1);
        assert_eq!(next_power_of_2(5), 8);
        assert_eq!(next_power_of_2(16), 16);
        assert_eq!(next_power_of_2(17), 32);
    }
    
    #[test]
    fn test_calculate_nrb() {
        // 20 MHz bandwidth with 30 kHz SCS
        assert_eq!(calculate_nrb(20_000_000, 30), 55);
        
        // 100 MHz bandwidth with 30 kHz SCS
        assert_eq!(calculate_nrb(100_000_000, 30), 277);
    }
    
    #[test]
    fn test_slot_duration() {
        assert_eq!(time::slot_duration_us(15), 1000);
        assert_eq!(time::slot_duration_us(30), 500);
        assert_eq!(time::slot_duration_us(120), 125);
    }
}