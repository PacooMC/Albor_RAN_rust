//! Resource Grid Mapping for 5G NR
//! 
//! Implements resource element mapping according to 3GPP TS 38.211

use crate::LayerError;
use common::types::{Bandwidth, SubcarrierSpacing};
use num_complex::Complex32;
use ndarray::Array2;
use tracing::{debug, warn, error, info};

/// Resource element in the grid
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ResourceElement {
    /// Subcarrier index (0 to N_RB * 12 - 1)
    pub subcarrier: u16,
    /// OFDM symbol index within slot (0 to 13 for normal CP)
    pub symbol: u8,
    /// Complex value
    pub value: Complex32,
}

/// Resource block
#[derive(Debug, Clone, Copy)]
pub struct ResourceBlock {
    /// RB index
    pub index: u16,
    /// Starting subcarrier
    pub start_subcarrier: u16,
}

impl ResourceBlock {
    /// Number of subcarriers per RB
    pub const SUBCARRIERS_PER_RB: u16 = 12;
    
    /// Create a new resource block
    pub fn new(index: u16) -> Self {
        Self {
            index,
            start_subcarrier: index * Self::SUBCARRIERS_PER_RB,
        }
    }
}

/// Resource grid for one slot
#[derive(Debug, Clone)]
pub struct ResourceGrid {
    /// 2D grid: [subcarriers, symbols]
    grid: Array2<Complex32>,
    /// FFT size
    fft_size: usize,
    /// Number of resource blocks
    num_rbs: u16,
    /// Number of used subcarriers
    num_subcarriers: u16,
    /// Number of symbols per slot
    symbols_per_slot: u8,
    /// DC subcarrier index
    dc_subcarrier: usize,
    /// Guard band size (one side)
    guard_band: usize,
}

impl ResourceGrid {
    /// Create a new resource grid
    pub fn new(
        fft_size: usize,
        symbols_per_slot: u8,
        bandwidth: Bandwidth,
        scs: SubcarrierSpacing,
    ) -> Result<Self, LayerError> {
        // Calculate number of RBs based on bandwidth
        let num_rbs = calculate_num_rbs(bandwidth, scs)?;
        let num_subcarriers = num_rbs * ResourceBlock::SUBCARRIERS_PER_RB;
        
        // Initialize grid with zeros
        debug!("Creating resource grid with dimensions: ({}, {})", fft_size, symbols_per_slot);
        let grid = Array2::zeros((fft_size, symbols_per_slot as usize));
        debug!("Grid created successfully, shape: {:?}", grid.shape());
        
        // Test that we can access the grid
        debug!("Testing grid access...");
        if let Some(elem) = grid.get((0, 0)) {
            debug!("Successfully accessed grid[0, 0] = {:?}", elem);
        } else {
            error!("Failed to access grid[0, 0] after creation!");
        }
        
        // Calculate DC subcarrier and guard band
        let dc_subcarrier = fft_size / 2;
        let guard_band = (fft_size - num_subcarriers as usize) / 2;
        
        Ok(Self {
            grid,
            fft_size,
            num_rbs,
            num_subcarriers,
            symbols_per_slot,
            dc_subcarrier,
            guard_band,
        })
    }
    
    /// Clear the entire grid
    pub fn clear(&mut self) {
        self.grid.fill(Complex32::new(0.0, 0.0));
    }
    
    /// Clear a specific symbol
    pub fn clear_symbol(&mut self, symbol: u8) {
        if symbol >= self.symbols_per_slot {
            warn!("Attempting to clear invalid symbol {} (max: {})", symbol, self.symbols_per_slot - 1);
            return;
        }
        
        let symbol_idx = symbol as usize;
        
        // Clear all elements in this column
        if let Some(mut column) = self.grid.column_mut(symbol_idx).into_slice() {
            for elem in column.iter_mut() {
                *elem = Complex32::new(0.0, 0.0);
            }
        } else {
            // Fallback: iterate manually with bounds checking
            let nrows = self.grid.nrows();
            let ncols = self.grid.ncols();
            
            if symbol_idx >= ncols {
                return;
            }
            
            for row in 0..nrows {
                if let Some(elem) = self.grid.get_mut((row, symbol_idx)) {
                    *elem = Complex32::new(0.0, 0.0);
                }
            }
        }
    }
    
    /// Map a resource element
    pub fn map_re(&mut self, subcarrier: u16, symbol: u8, value: Complex32) -> Result<(), LayerError> {
        if subcarrier >= self.num_subcarriers || symbol >= self.symbols_per_slot {
            return Err(LayerError::InvalidConfiguration(
                format!("RE out of bounds: subcarrier={}, symbol={}", subcarrier, symbol)
            ));
        }
        
        // Convert absolute subcarrier index to DC-relative
        let dc_offset = self.num_subcarriers as i16 / 2;
        let dc_relative_sc = subcarrier as i16 - dc_offset;
        let fft_index = self.subcarrier_to_fft_index(dc_relative_sc);
        
        if fft_index >= self.fft_size {
            return Err(LayerError::InvalidConfiguration(
                format!("FFT index {} out of bounds (max {}), subcarrier={}", 
                       fft_index, self.fft_size-1, subcarrier)
            ));
        }
        
        self.grid[(fft_index, symbol as usize)] = value;
        Ok(())
    }
    
    /// Get a resource element
    pub fn get_re(&self, subcarrier: u16, symbol: u8) -> Option<Complex32> {
        if subcarrier >= self.num_subcarriers || symbol >= self.symbols_per_slot {
            return None;
        }
        
        // Convert absolute subcarrier index to DC-relative
        let dc_offset = self.num_subcarriers as i16 / 2;
        let dc_relative_sc = subcarrier as i16 - dc_offset;
        let fft_index = self.subcarrier_to_fft_index(dc_relative_sc);
        
        if fft_index >= self.fft_size {
            return None;
        }
        
        Some(self.grid[(fft_index, symbol as usize)])
    }
    
    /// Map resource block
    pub fn map_rb(&mut self, rb_index: u16, symbol: u8, values: &[Complex32; 12]) -> Result<(), LayerError> {
        if rb_index >= self.num_rbs {
            return Err(LayerError::InvalidConfiguration(
                format!("RB index {} out of bounds", rb_index)
            ));
        }
        
        let rb = ResourceBlock::new(rb_index);
        for (i, &value) in values.iter().enumerate() {
            self.map_re(rb.start_subcarrier + i as u16, symbol, value)?;
        }
        
        Ok(())
    }
    
    /// Map PSS (Primary Synchronization Signal)
    pub fn map_pss(&mut self, symbol: u8, pss_sequence: &[Complex32]) -> Result<(), LayerError> {
        // PSS occupies 127 subcarriers within the SSB
        if pss_sequence.len() != 127 {
            return Err(LayerError::InvalidConfiguration(
                format!("PSS sequence length must be 127, got {}", pss_sequence.len())
            ));
        }
        
        // According to 3GPP TS 38.211, PSS starts at subcarrier 56 within the SSB
        // SSB is 240 subcarriers wide, centered around the SSB center frequency
        // For now, we center SSB around DC, but this should be configurable
        let ssb_start_sc = -(240 / 2) as i16;  // SSB starts at -120 from DC
        let pss_start_within_ssb = 56;  // PSS starts at subcarrier 56 within SSB
        let pss_start_sc = ssb_start_sc + pss_start_within_ssb;
        
        // OPTIMIZATION: Removed logging from hot path
        let mut mapped_count = 0;
        for (i, &value) in pss_sequence.iter().enumerate() {
            let sc = pss_start_sc + i as i16;
            let fft_index = self.subcarrier_to_fft_index(sc);
            if fft_index >= self.fft_size {
                error!("FFT index {} out of bounds (max {}), subcarrier={}", 
                       fft_index, self.fft_size-1, sc);
                return Err(LayerError::InvalidConfiguration(
                    format!("FFT index out of bounds")
                ));
            }
            self.grid[(fft_index, symbol as usize)] = value;
            mapped_count += 1;
        }
        // PSS mapping complete
        
        Ok(())
    }
    
    /// Map SSS (Secondary Synchronization Signal)
    pub fn map_sss(&mut self, symbol: u8, sss_sequence: &[Complex32]) -> Result<(), LayerError> {
        // SSS also occupies 127 subcarriers within the SSB
        if sss_sequence.len() != 127 {
            return Err(LayerError::InvalidConfiguration(
                format!("SSS sequence length must be 127, got {}", sss_sequence.len())
            ));
        }
        
        // SSS has the same position as PSS within the SSB
        let ssb_start_sc = -(240 / 2) as i16;  // SSB starts at -120 from DC
        let sss_start_within_ssb = 56;  // SSS starts at subcarrier 56 within SSB
        let sss_start_sc = ssb_start_sc + sss_start_within_ssb;
        
        debug!("Mapping SSS: starts at {} (within SSB: {}), symbol={}", 
               sss_start_sc, sss_start_within_ssb, symbol);
        
        for (i, &value) in sss_sequence.iter().enumerate() {
            let sc = sss_start_sc + i as i16;
            let fft_index = self.subcarrier_to_fft_index(sc);
            self.grid[(fft_index, symbol as usize)] = value;
        }
        
        Ok(())
    }
    
    /// Map PBCH (Physical Broadcast Channel)
    pub fn map_pbch(&mut self, symbol: u8, pbch_symbols: &[Complex32]) -> Result<(), LayerError> {
        // PBCH occupies 240 subcarriers (20 RBs) centered around DC
        if pbch_symbols.len() != 240 {
            return Err(LayerError::InvalidConfiguration(
                format!("PBCH symbols length must be 240, got {}", pbch_symbols.len())
            ));
        }
        
        // Map PBCH centered around DC
        let start_sc = -(240 / 2) as i16;
        for (i, &value) in pbch_symbols.iter().enumerate() {
            let sc = start_sc + i as i16;
            let fft_index = self.subcarrier_to_fft_index(sc);
            self.grid[(fft_index, symbol as usize)] = value;
        }
        
        Ok(())
    }
    
    /// Map DMRS (Demodulation Reference Signal) for PBCH
    pub fn map_pbch_dmrs(&mut self, symbol: u8, cell_id: u16) -> Result<(), LayerError> {
        // PBCH DMRS pattern depends on cell ID
        // According to 3GPP TS 38.211, v = N_cell_ID mod 4
        let v = (cell_id % 4) as usize;
        
        // Generate DMRS sequence
        let dmrs_sequence = generate_pbch_dmrs(cell_id);
        
        // SSB/PBCH spans 240 subcarriers (20 RBs)
        let ssb_start_sc = -(240 / 2) as i16;
        
        // DMRS is placed at positions v, v+4, v+8 within each group of 12 subcarriers
        let mut dmrs_idx = 0;
        for rb in 0..20 {  // 20 RBs in SSB
            for pos in [v, v + 4, v + 8].iter() {
                if *pos < 12 && dmrs_idx < dmrs_sequence.len() {
                    let sc = ssb_start_sc + (rb * 12 + *pos) as i16;
                    let fft_index = self.subcarrier_to_fft_index(sc);
                    self.grid[(fft_index, symbol as usize)] = dmrs_sequence[dmrs_idx];
                    dmrs_idx += 1;
                }
            }
        }
        
        debug!("Mapped {} PBCH DMRS symbols with v={} at symbol {}", dmrs_idx, v, symbol);
        Ok(())
    }
    
    /// Get symbol for OFDM modulation
    pub fn get_symbol(&self, symbol: u8) -> Vec<Complex32> {
        if symbol >= self.symbols_per_slot {
            return vec![Complex32::new(0.0, 0.0); self.fft_size];
        }
        
        self.grid.column(symbol as usize).to_vec()
    }
    
    /// Get symbol data as a slice view (no copy)
    pub fn get_symbol_view(&self, symbol: u8) -> Option<ndarray::ArrayView1<Complex32>> {
        if symbol >= self.symbols_per_slot {
            return None;
        }
        
        Some(self.grid.column(symbol as usize))
    }
    
    /// Set symbol from OFDM demodulation
    pub fn set_symbol(&mut self, symbol: u8, data: &[Complex32]) -> Result<(), LayerError> {
        if symbol >= self.symbols_per_slot {
            return Err(LayerError::InvalidConfiguration(
                format!("Symbol {} out of bounds", symbol)
            ));
        }
        
        if data.len() != self.fft_size {
            return Err(LayerError::InvalidConfiguration(
                format!("Data length {} doesn't match FFT size {}", data.len(), self.fft_size)
            ));
        }
        
        self.grid.column_mut(symbol as usize).assign(&ndarray::ArrayView1::from(data));
        Ok(())
    }
    
    /// Convert logical subcarrier index to FFT bin index
    fn subcarrier_to_fft_index(&self, subcarrier: i16) -> usize {
        // DC is at fft_size/2
        // Positive frequencies: DC to fft_size/2-1
        // Negative frequencies: fft_size/2 to fft_size-1 (wraps around)
        let dc_idx = self.fft_size / 2;
        
        if subcarrier >= 0 {
            // Positive frequencies
            dc_idx + subcarrier as usize
        } else {
            // Negative frequencies wrap around
            (self.fft_size as i32 + subcarrier as i32) as usize
        }
    }
}

/// Calculate number of resource blocks for given bandwidth
fn calculate_num_rbs(bandwidth: Bandwidth, scs: SubcarrierSpacing) -> Result<u16, LayerError> {
    // Number of RBs based on 3GPP TS 38.104 Table 5.3.2-1
    let num_rbs = match (bandwidth, scs) {
        (Bandwidth::Bw5, SubcarrierSpacing::Scs15) => 25,
        (Bandwidth::Bw5, SubcarrierSpacing::Scs30) => 11,
        (Bandwidth::Bw10, SubcarrierSpacing::Scs15) => 52,
        (Bandwidth::Bw10, SubcarrierSpacing::Scs30) => 24,
        (Bandwidth::Bw10, SubcarrierSpacing::Scs60) => 11,
        (Bandwidth::Bw15, SubcarrierSpacing::Scs15) => 79,
        (Bandwidth::Bw15, SubcarrierSpacing::Scs30) => 38,
        (Bandwidth::Bw15, SubcarrierSpacing::Scs60) => 18,
        (Bandwidth::Bw20, SubcarrierSpacing::Scs15) => 106,
        (Bandwidth::Bw20, SubcarrierSpacing::Scs30) => 51,
        (Bandwidth::Bw20, SubcarrierSpacing::Scs60) => 24,
        (Bandwidth::Bw25, SubcarrierSpacing::Scs15) => 133,
        (Bandwidth::Bw25, SubcarrierSpacing::Scs30) => 65,
        (Bandwidth::Bw25, SubcarrierSpacing::Scs60) => 31,
        (Bandwidth::Bw30, SubcarrierSpacing::Scs15) => 160,
        (Bandwidth::Bw30, SubcarrierSpacing::Scs30) => 78,
        (Bandwidth::Bw30, SubcarrierSpacing::Scs60) => 38,
        (Bandwidth::Bw40, SubcarrierSpacing::Scs15) => 216,
        (Bandwidth::Bw40, SubcarrierSpacing::Scs30) => 106,
        (Bandwidth::Bw40, SubcarrierSpacing::Scs60) => 51,
        (Bandwidth::Bw50, SubcarrierSpacing::Scs15) => 270,
        (Bandwidth::Bw50, SubcarrierSpacing::Scs30) => 133,
        (Bandwidth::Bw50, SubcarrierSpacing::Scs60) => 65,
        (Bandwidth::Bw50, SubcarrierSpacing::Scs120) => 31,
        (Bandwidth::Bw60, SubcarrierSpacing::Scs30) => 162,
        (Bandwidth::Bw60, SubcarrierSpacing::Scs60) => 79,
        (Bandwidth::Bw60, SubcarrierSpacing::Scs120) => 38,
        (Bandwidth::Bw80, SubcarrierSpacing::Scs30) => 217,
        (Bandwidth::Bw80, SubcarrierSpacing::Scs60) => 107,
        (Bandwidth::Bw80, SubcarrierSpacing::Scs120) => 51,
        (Bandwidth::Bw100, SubcarrierSpacing::Scs30) => 273,
        (Bandwidth::Bw100, SubcarrierSpacing::Scs60) => 135,
        (Bandwidth::Bw100, SubcarrierSpacing::Scs120) => 65,
        _ => return Err(LayerError::InvalidConfiguration(
            format!("Invalid bandwidth {:?} and SCS {:?} combination", bandwidth, scs)
        )),
    };
    
    Ok(num_rbs)
}

/// Generate PBCH DMRS sequence
fn generate_pbch_dmrs(cell_id: u16) -> Vec<Complex32> {
    // Simplified DMRS generation
    // In real implementation, this should use Gold sequence based on cell ID
    let mut dmrs = Vec::with_capacity(60);
    let init = 2u32.pow(10) * (7 * (4 + 1) + 1) + 2 * cell_id as u32 + 1;
    
    // Simple pseudo-random sequence
    let mut x = init;
    for _ in 0..60 {
        x = (x.wrapping_mul(1103515245).wrapping_add(12345)) & 0x7fffffff;
        let bit = (x >> 16) & 1;
        let value = if bit == 1 {
            Complex32::new(1.0 / 2.0_f32.sqrt(), 1.0 / 2.0_f32.sqrt())
        } else {
            Complex32::new(-1.0 / 2.0_f32.sqrt(), -1.0 / 2.0_f32.sqrt())
        };
        dmrs.push(value);
    }
    
    dmrs
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_resource_grid_creation() {
        let grid = ResourceGrid::new(
            2048,
            14,
            Bandwidth::Bw20,
            SubcarrierSpacing::Scs15,
        ).unwrap();
        
        assert_eq!(grid.num_rbs, 106);
        assert_eq!(grid.num_subcarriers, 106 * 12);
        assert_eq!(grid.fft_size, 2048);
    }
    
    #[test]
    fn test_resource_element_mapping() {
        let mut grid = ResourceGrid::new(
            2048,
            14,
            Bandwidth::Bw20,
            SubcarrierSpacing::Scs15,
        ).unwrap();
        
        let value = Complex32::new(1.0, 0.0);
        grid.map_re(0, 0, value).unwrap();
        
        let retrieved = grid.get_re(0, 0).unwrap();
        assert_eq!(retrieved, value);
    }
    
    #[test]
    fn test_rb_mapping() {
        let mut grid = ResourceGrid::new(
            2048,
            14,
            Bandwidth::Bw20,
            SubcarrierSpacing::Scs15,
        ).unwrap();
        
        let values = [Complex32::new(1.0, 0.0); 12];
        grid.map_rb(0, 0, &values).unwrap();
        
        for i in 0..12 {
            let retrieved = grid.get_re(i, 0).unwrap();
            assert_eq!(retrieved, Complex32::new(1.0, 0.0));
        }
    }
}