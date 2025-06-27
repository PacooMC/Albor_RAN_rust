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
    /// Cell ID for DMRS generation
    cell_id: u16,
}

impl ResourceGrid {
    /// Create a new resource grid
    pub fn new(
        fft_size: usize,
        symbols_per_slot: u8,
        bandwidth: Bandwidth,
        scs: SubcarrierSpacing,
    ) -> Result<Self, LayerError> {
        Self::new_with_cell_id(fft_size, symbols_per_slot, bandwidth, scs, 0)
    }
    
    /// Create a new resource grid with cell ID
    pub fn new_with_cell_id(
        fft_size: usize,
        symbols_per_slot: u8,
        bandwidth: Bandwidth,
        scs: SubcarrierSpacing,
        cell_id: u16,
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
            cell_id,
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
    
    /// Map PSS (Primary Synchronization Signal) with k_SSB offset
    pub fn map_pss(&mut self, symbol: u8, pss_sequence: &[Complex32], k_ssb: i16) -> Result<(), LayerError> {
        // PSS occupies 127 subcarriers within the SSB
        if pss_sequence.len() != 127 {
            return Err(LayerError::InvalidConfiguration(
                format!("PSS sequence length must be 127, got {}", pss_sequence.len())
            ));
        }
        
        // According to 3GPP TS 38.211, PSS starts at subcarrier 56 within the SSB
        // SSB is 240 subcarriers wide
        // k_SSB is the offset to the first subcarrier of the SSB (not the center!)
        let ssb_start_sc = k_ssb;  // k_SSB points to first subcarrier of SSB
        let pss_start_within_ssb = 56;  // PSS starts at subcarrier 56 within SSB
        let pss_start_sc = ssb_start_sc + pss_start_within_ssb;
        
        info!("Mapping PSS to resource grid:");
        info!("  k_SSB: {} subcarriers", k_ssb);
        info!("  SSB starts at subcarrier: {}", ssb_start_sc);
        info!("  PSS starts within SSB at: {}", pss_start_within_ssb);
        info!("  PSS absolute start subcarrier: {}", pss_start_sc);
        info!("  PSS ends at subcarrier: {}", pss_start_sc + 126);
        info!("  Symbol: {}", symbol);
        
        // Log first few PSS values being mapped
        info!("First 5 PSS values being mapped:");
        for i in 0..5.min(pss_sequence.len()) {
            info!("  PSS[{}] = {:.3} + {:.3}j -> subcarrier {}", 
                  i, pss_sequence[i].re, pss_sequence[i].im, pss_start_sc + i as i16);
        }
        
        // Calculate signal power before mapping
        let input_power: f32 = pss_sequence.iter().map(|s| s.norm_sqr()).sum::<f32>() / pss_sequence.len() as f32;
        let input_power_db = 10.0 * input_power.log10();
        let input_peak = pss_sequence.iter().map(|s| s.norm()).max_by(|a, b| a.partial_cmp(b).unwrap()).unwrap_or(0.0);
        info!("PSS input signal: power={:.2} dB, peak={:.3}, RMS={:.4}", 
              input_power_db, input_peak, input_power.sqrt());
        
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
            
            // Log mapping details for first few samples
            if i < 5 {
                info!("  Mapped PSS[{}] = {:.3}+{:.3}j to FFT bin {} (subcarrier {})", 
                      i, value.re, value.im, fft_index, sc);
            }
        }
        info!("PSS mapping complete: {} subcarriers mapped to resource grid", mapped_count);
        
        Ok(())
    }
    
    /// Map SSS (Secondary Synchronization Signal) with k_SSB offset
    pub fn map_sss(&mut self, symbol: u8, sss_sequence: &[Complex32], k_ssb: i16) -> Result<(), LayerError> {
        // SSS also occupies 127 subcarriers within the SSB
        if sss_sequence.len() != 127 {
            return Err(LayerError::InvalidConfiguration(
                format!("SSS sequence length must be 127, got {}", sss_sequence.len())
            ));
        }
        
        // SSS has the same position as PSS within the SSB
        // k_SSB points to first subcarrier of SSB
        let ssb_start_sc = k_ssb;  // SSB starts at k_SSB
        let sss_start_within_ssb = 56;  // SSS starts at subcarrier 56 within SSB
        let sss_start_sc = ssb_start_sc + sss_start_within_ssb;
        
        debug!("Mapping SSS: starts at {} (within SSB: {}), symbol={}, k_SSB={}", 
               sss_start_sc, sss_start_within_ssb, symbol, k_ssb);
        
        for (i, &value) in sss_sequence.iter().enumerate() {
            let sc = sss_start_sc + i as i16;
            let fft_index = self.subcarrier_to_fft_index(sc);
            self.grid[(fft_index, symbol as usize)] = value;
        }
        
        Ok(())
    }
    
    /// Map PBCH (Physical Broadcast Channel) with k_SSB offset
    /// The pbch_symbols contains 432 QPSK symbols that need to be distributed across 3 OFDM symbols
    /// This function maps the appropriate portion based on the relative symbol position and actual grid symbol
    pub fn map_pbch(&mut self, relative_symbol: u8, actual_symbol: u8, pbch_symbols: &[Complex32], k_ssb: i16) -> Result<(), LayerError> {
        // PBCH has 432 QPSK symbols distributed across 3 OFDM symbols
        if pbch_symbols.len() != 432 {
            return Err(LayerError::InvalidConfiguration(
                format!("PBCH symbols length must be 432, got {}", pbch_symbols.len())
            ));
        }
        
        // Map PBCH with k_SSB offset
        // k_SSB points to first subcarrier of SSB
        let start_sc = k_ssb;  // PBCH starts at first subcarrier of SSB
        
        // Map PBCH based on relative position within SSB
        // SSB structure: PSS(0), PBCH(1), SSS(2), PBCH(3)
        // 432 QPSK symbols are distributed as:
        // - Symbol 1: 216 symbols (240 subcarriers - 24 DMRS)
        // - Symbol 2: 96 symbols (48+48 edge subcarriers, avoiding SSS)
        // - Symbol 3: 216 symbols (240 subcarriers - 24 DMRS)
        
        // Calculate symbol offset for extracting the right portion
        // PBCH mapping in SSB: relative positions 1 and 3 (absolute symbols vary)
        // Symbol 2 (relative) is SSS position where PBCH maps to edges
        // For our 432 symbol distribution:
        // - First PBCH (relative 1): 216 symbols
        // - SSS position (relative 2): 96 symbols on edges  
        // - Second PBCH (relative 3): 120 symbols
        let (symbol_offset, num_symbols) = match relative_symbol {
            1 => (0, 216),      // First PBCH symbol: symbols 0-215
            2 => (216, 96),     // SSS symbol with PBCH edges: symbols 216-311
            3 => (312, 120),    // Second PBCH symbol: symbols 312-431
            _ => {
                warn!("Invalid PBCH relative symbol position: {}, expected 1, 2, or 3", relative_symbol);
                return Err(LayerError::InvalidConfiguration(
                    format!("Invalid PBCH relative symbol position: {}", relative_symbol)
                ));
            }
        };
        
        // Extract the portion of PBCH symbols for this OFDM symbol
        let end_idx = (symbol_offset + num_symbols).min(pbch_symbols.len());
        let symbol_portion = &pbch_symbols[symbol_offset..end_idx];
        
        if relative_symbol == 2 {
            // Symbol 2 (SSS position) - map PBCH to edges only
            // Lower edge: subcarriers 0-47 within SSB
            let lower_symbols = symbol_portion.len().min(48);
            for i in 0..lower_symbols {
                let sc = start_sc + i as i16;
                let fft_index = self.subcarrier_to_fft_index(sc);
                self.grid[(fft_index, actual_symbol as usize)] = symbol_portion[i];
            }
            
            // Upper edge: subcarriers 192-239 within SSB
            if symbol_portion.len() > 48 {
                let upper_start = 48;
                let upper_symbols = (symbol_portion.len() - 48).min(48);
                for i in 0..upper_symbols {
                    let sc = start_sc + (192 + i) as i16;
                    let fft_index = self.subcarrier_to_fft_index(sc);
                    self.grid[(fft_index, actual_symbol as usize)] = symbol_portion[upper_start + i];
                }
            }
            
            debug!("Mapped {} PBCH symbols in SSS symbol position (rel={}, actual={}): edges only", 
                   symbol_portion.len(), relative_symbol, actual_symbol);
        } else {
            // Symbols 1 and 3: Map to all subcarriers except DMRS positions
            // DMRS is on every 4th subcarrier starting from v = cell_id % 4
            // For now, use v=0 as we'll handle DMRS separately
            let v = 0usize;
            let mut pbch_idx = 0;
            
            for k in 0..240 {
                // Skip DMRS positions
                if (k >= v) && ((k - v) % 4 == 0) {
                    continue;  // This is a DMRS position
                }
                
                if pbch_idx < symbol_portion.len() {
                    let sc = start_sc + k as i16;
                    let fft_index = self.subcarrier_to_fft_index(sc);
                    self.grid[(fft_index, actual_symbol as usize)] = symbol_portion[pbch_idx];
                    pbch_idx += 1;
                }
            }
            
            debug!("Mapped {} PBCH symbols to symbol {} (relative {}): data subcarriers only", 
                   pbch_idx, actual_symbol, relative_symbol);
        }
        
        Ok(())
    }
    
    /// Map DMRS (Demodulation Reference Signal) for PBCH with k_SSB offset
    pub fn map_pbch_dmrs(&mut self, relative_symbol: u8, actual_symbol: u8, cell_id: u16, k_ssb: i16, 
                         ssb_idx: u8, frame_number: u32) -> Result<(), LayerError> {
        // PBCH DMRS pattern depends on cell ID
        // According to 3GPP TS 38.211, v = N_cell_ID mod 4
        let v = (cell_id % 4) as usize;
        
        // Generate DMRS sequence with proper parameters
        let l_max = 4u8; // Default L_max = 4 for FR1
        let dmrs_sequence = generate_pbch_dmrs(cell_id, ssb_idx, frame_number, l_max);
        
        // SSB/PBCH spans 240 subcarriers (20 RBs) with k_SSB offset
        // k_SSB points to first subcarrier of SSB
        let ssb_start_sc = k_ssb;  // SSB starts at k_SSB
        
        let mut dmrs_idx = 0;
        
        // Symbol-specific mapping according to 3GPP TS 38.211
        match relative_symbol {
            1 | 3 => {
                // Symbols 1 and 3: Map across all 240 subcarriers
                for k in (v..240).step_by(4) {
                    if dmrs_idx < dmrs_sequence.len() {
                        let sc = ssb_start_sc + k as i16;
                        let fft_index = self.subcarrier_to_fft_index(sc);
                        self.grid[(fft_index, actual_symbol as usize)] = dmrs_sequence[dmrs_idx];
                        dmrs_idx += 1;
                    }
                }
            }
            2 => {
                // Symbol 2: Map only in lower (0-47) and upper (192-239) regions
                // This avoids the SSS region (48-191)
                
                // Lower section (0-47)
                for k in (v..48).step_by(4) {
                    if dmrs_idx < dmrs_sequence.len() {
                        let sc = ssb_start_sc + k as i16;
                        let fft_index = self.subcarrier_to_fft_index(sc);
                        self.grid[(fft_index, actual_symbol as usize)] = dmrs_sequence[dmrs_idx];
                        dmrs_idx += 1;
                    }
                }
                
                // Upper section (192-239)
                for k in ((192 + v)..240).step_by(4) {
                    if dmrs_idx < dmrs_sequence.len() {
                        let sc = ssb_start_sc + k as i16;
                        let fft_index = self.subcarrier_to_fft_index(sc);
                        self.grid[(fft_index, actual_symbol as usize)] = dmrs_sequence[dmrs_idx];
                        dmrs_idx += 1;
                    }
                }
            }
            _ => {
                return Err(LayerError::InvalidConfiguration(
                    format!("Invalid PBCH DMRS relative symbol: {}", relative_symbol)
                ));
            }
        }
        
        debug!("Mapped {} PBCH DMRS symbols with v={} at symbol {} (relative {}), k_SSB={}, SSB idx={}", 
               dmrs_idx, v, actual_symbol, relative_symbol, k_ssb, ssb_idx);
        Ok(())
    }
    
    /// Get symbol for OFDM modulation
    pub fn get_symbol(&self, symbol: u8) -> Vec<Complex32> {
        if symbol >= self.symbols_per_slot {
            return vec![Complex32::new(0.0, 0.0); self.fft_size];
        }
        
        let samples = self.grid.column(symbol as usize).to_vec();
        
        // Debug: count non-zero samples
        let non_zero_count = samples.iter().filter(|s| s.norm_sqr() > 0.0).count();
        if non_zero_count > 0 {
            debug!("Resource grid symbol {}: {} non-zero subcarriers out of {}", 
                   symbol, non_zero_count, self.fft_size);
        }
        
        samples
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

/// Generate PBCH DMRS sequence using Gold sequence
fn generate_pbch_dmrs(cell_id: u16, ssb_idx: u8, frame_number: u32, l_max: u8) -> Vec<Complex32> {
    use crate::phy::dmrs::{DmrsSequenceGenerator, calculate_pbch_dmrs_cinit};
    
    // Calculate half frame number (n_hf)
    let n_hf = ((frame_number / 5) % 2) as u8;
    
    // Calculate initialization value for Gold sequence
    let c_init = calculate_pbch_dmrs_cinit(cell_id, ssb_idx, n_hf, l_max);
    
    // Create Gold sequence generator
    let mut generator = DmrsSequenceGenerator::new(c_init);
    
    // Generate 144 QPSK symbols for PBCH DMRS
    // Power normalization: 1/sqrt(2) for unit power QPSK
    let amplitude = 1.0 / std::f32::consts::SQRT_2;
    let mut dmrs = Vec::with_capacity(144);
    
    for _ in 0..144 {
        dmrs.push(generator.next_qpsk_symbol(amplitude));
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