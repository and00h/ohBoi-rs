/// Module for handling Direct Memory Access (DMA) operations.
pub(crate) mod dma;

/// Module for handling cartridge-related operations.
pub mod cartridge;

/// The size of a memory bank in bytes.
const BANK_SIZE: usize = 0x1000;

/// Represents the Work RAM (WRAM) structure.
///
/// This structure manages the memory and bank switching for the WRAM.
pub(crate) struct WRAM {
    /// The memory storage for WRAM.
    mem: Vec<u8>,
    /// The index of the currently active bank for bank 1.
    pub(crate) bank1_index: usize,
}

impl WRAM {
    /// Creates a new `WRAM` instance with initialized memory.
    ///
    /// # Returns
    ///
    /// A new `WRAM` instance with all memory set to zero and bank 1 set to index 1.
    pub fn new() -> Self {
        WRAM {
            mem: vec![0; BANK_SIZE * 8],
            bank1_index: 1,
        }
    }

    /// Reads a byte from the specified address in WRAM.
    ///
    /// # Arguments
    ///
    /// * `addr` - The address to read from.
    ///
    /// # Returns
    ///
    /// The value at the specified address.
    ///
    /// # Panics
    ///
    /// Panics if the address is outside the valid WRAM range.
    pub fn read(&self, addr: u16) -> u8 {
        let index = addr as usize & 0xFFF;
        match addr {
            0xC000..=0xCFFF => self.mem[index],
            0xD000..=0xDFFF => self.mem[BANK_SIZE * self.bank1_index + index],
            _ => panic!("Invalid address {}!", addr),
        }
    }

    /// Writes a byte to the specified address in WRAM.
    ///
    /// # Arguments
    ///
    /// * `addr` - The address to write to.
    /// * `val` - The value to write.
    ///
    /// # Panics
    ///
    /// Panics if the address is outside the valid WRAM range.
    pub fn write(&mut self, addr: u16, val: u8) {
        let index = addr as usize & 0xFFF;
        match addr {
            0xC000..=0xCFFF => self.mem[index] = val,
            0xD000..=0xDFFF => self.mem[BANK_SIZE * self.bank1_index + index] = val,
            _ => panic!("Invalid address {}!", addr),
        }
    }

    /// Switches the active bank for bank 1.
    ///
    /// # Arguments
    ///
    /// * `new_bank` - The index of the new bank to switch to. If `new_bank` is 0, it defaults to 1.
    pub fn switch_bank(&mut self, new_bank: usize) {
        self.bank1_index = if new_bank == 0 { 1 } else { new_bank & 0b111 };
    }
}