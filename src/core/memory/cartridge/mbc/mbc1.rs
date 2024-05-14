use log::warn;
use crate::core::memory::cartridge::CartridgeHeader;
use crate::core::memory::cartridge::mbc::{BankingMode, RAM_BANK_SIZE, ROM_BANK_SIZE};
use super::Mbc;

pub(super) struct Mbc1 {
    rom: Vec<u8>,
    ram: Option<Vec<u8>>,
    ram_enabled: bool,
    banking_mode: BankingMode,
    battery: bool,
    rom_bank_hi: usize,
    rom_bank_lo: usize,
    n_rom_banks: usize,
}

impl Mbc1 {
    pub fn new(rom: Vec<u8>, cart_header: &CartridgeHeader, sav: Option<Vec<u8>>, battery: bool) -> Self {
        Self {
            rom,
            ram: match sav {
                Some(sav) => Some(sav),
                None if cart_header.ram_size != 0 => Some(vec![0; cart_header.ram_size]),
                _ => None
            },
            ram_enabled: false,
            banking_mode: BankingMode::ROM,
            battery,
            rom_bank_hi: 0,
            rom_bank_lo: 1,
            n_rom_banks: cart_header.rom_size / ROM_BANK_SIZE,
        }
    }
}

impl Mbc for Mbc1 {
    fn read(&self, addr: u16) -> u8 {
        let mut bank_number = if addr < 0x4000 { 0 } else { ((self.rom_bank_hi << 5) | self.rom_bank_lo) & 0x7F };
        if bank_number == 0 && matches!(self.banking_mode, BankingMode::RAM) {
            bank_number = self.rom_bank_hi << 5;
        }
        bank_number %= self.n_rom_banks;
        let bank_offset = (addr as usize) % ROM_BANK_SIZE;
        let effective_address = ROM_BANK_SIZE * bank_number + bank_offset;

        self.rom[effective_address]
    }

    fn write(&mut self, addr: u16, val: u8) {
        let val = val as usize;
        match addr {
            0..=0x1FFF => self.ram_enabled = (val & 0xF == 0xA),
            0x2000..=0x3FFF => self.rom_bank_lo = if val & 0x1F == 0 { 1 } else { val & 0x1F },
            0x4000..=0x5FFF => self.rom_bank_hi = val & 0x3,
            0x6000..=0x7FFF if val == 0 => self.banking_mode = BankingMode::RAM,
            0x6000..=0x7FFF => self.banking_mode = BankingMode::ROM,
            _ => panic!("Invalid write to cartridge at address {:04X}", addr)
        }
    }

    fn read_ext_ram(&self, addr: u16) -> u8 {
        match self.ram {
            Some(ref ram) => {
                let bank = if matches!(self.banking_mode, BankingMode::RAM) { self.rom_bank_hi } else { 0 };
                let addr = bank * RAM_BANK_SIZE + (addr as usize - 0xA000);
                ram[addr]
            },
            None => {
                warn!("Tried reading from external RAM when cartridge has none");
                0xFF
            }
        }
    }

    fn write_ext_ram(&mut self, addr: u16, val: u8) {
        match self.ram {
            Some(ref mut ram) => {
                let bank = if matches!(self.banking_mode, BankingMode::RAM) { self.rom_bank_hi } else { 0 };
                let addr = bank * RAM_BANK_SIZE + (addr as usize - 0xA000);
                ram[addr] = val;
            },
            None => warn!("Tried writing to external RAM when cartridge has none")
        }
    }

    fn has_battery(&self) -> bool {
        self.battery
    }
    fn has_ram(&self) -> bool {
        matches!(self.ram, Some(_))
    }

    fn ram(&self) -> Option<&Vec<u8>> {
        self.ram.as_ref()
    }

    fn rom(&self) -> &Vec<u8> {
        &self.rom
    }
}