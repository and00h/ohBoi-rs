pub(crate) mod dma;
pub mod cartridge;

const BANK_SIZE: usize = 0x1000;

pub(crate) struct WRAM {
    mem: Vec<u8>,
    bank1_index: usize,
}

impl WRAM {
    pub fn new() -> Self {
        WRAM {
            mem: vec![0; BANK_SIZE * 8],
            bank1_index: 1
        }
    }

    pub fn read(&self, addr: u16) -> u8 {
        let index = addr as usize & 0xFFF;
        match addr {
            0xC000..=0xCFFF => self.mem[index],
            0xD000..=0xDFFF => self.mem[BANK_SIZE * self.bank1_index + index],
            _ => panic!("Invalid address {}!", addr)
        }
    }

    pub fn write(&mut self, addr: u16, val: u8) {
        let index = addr as usize & 0xFFF;
        match addr {
            0xC000..=0xCFFF => self.mem[index] = val,
            0xD000..=0xDFFF => self.mem[BANK_SIZE * self.bank1_index + index] = val,
            _ => panic!("Invalid address {}!", addr)
        }
    }

    pub fn switch_bank(&mut self, new_bank: usize) {
        self.bank1_index = if new_bank == 0 { 1 } else { new_bank & 0b111 };
    }
}