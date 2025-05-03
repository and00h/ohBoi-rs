// Copyright Antonio Porsia 2025. Licensed under the EUPL-1.2 or later.

use log::debug;
use crate::memory::cartridge::CartridgeHeader;
use super::{Mbc, RAM_BANK_SIZE, ROM_BANK_SIZE};

pub(crate) struct Mbc5 {
    rom: Vec<u8>,
    ram: Option<Vec<u8>>,
    ram_enabled: bool,
    ram_bank: usize,
    battery: bool,
    rom_bank_hi: usize,
    rom_bank_lo: usize,
    n_rom_banks: usize,
    n_ram_banks: usize
}

impl Mbc5 {
    pub fn new(rom: Vec<u8>, cart_header: &CartridgeHeader, sav: Option<Vec<u8>>, battery: bool) -> Self {
        Self {
            rom,
            ram: match sav {
                Some(sav) => Some(sav),
                None if cart_header.ram_size != 0 => Some(vec![0; cart_header.ram_size]),
                _ => None
            },
            ram_enabled: false,
            ram_bank: 0,
            battery,
            rom_bank_hi: 0,
            rom_bank_lo: 1,
            n_rom_banks: cart_header.rom_size / ROM_BANK_SIZE,
            n_ram_banks: cart_header.ram_size / RAM_BANK_SIZE
        }
    }
}

impl Mbc for Mbc5 {
    fn read(&self, addr: u16) -> u8 {
        let mut bank_number = if addr < 0x4000 { 0 } else { (self.rom_bank_hi << 8) | self.rom_bank_lo };

        bank_number %= self.n_rom_banks;
        let bank_offset = (addr as usize) % ROM_BANK_SIZE;
        let effective_address = ROM_BANK_SIZE * bank_number + bank_offset;

        self.rom[effective_address]
    }

    fn write(&mut self, addr: u16, val: u8) {
        let val = val as usize;
        match addr {
            0..=0x1FFF => self.ram_enabled = val & 0xF == 0xA,
            0x2000..=0x2FFF => self.rom_bank_lo = val,
            0x3000..=0x3FFF => self.rom_bank_hi = val & 0x1,
            0x4000..=0x5FFF => self.ram_bank = val & 0xF,
            _ => debug!("Invalid write to cartridge at address {:04X}", addr)
        }
    }

    fn read_ext_ram(&self, addr: u16) -> u8 {
        match self.ram {
            Some(ref ram) if self.ram_enabled => {
                let addr = self.ram_bank * RAM_BANK_SIZE + (addr as usize - 0xA000);
                ram[addr]
            },
            None | Some(_) => {
                0xFF
            }
        }
    }

    fn write_ext_ram(&mut self, addr: u16, val: u8) {
        match self.ram {
            Some(ref mut ram) if self.ram_enabled => {
                let addr = self.ram_bank * RAM_BANK_SIZE + (addr as usize - 0xA000);
                ram[addr] = val;
            },
            _ => {} // warn!("Tried writing to external RAM when cartridge has none")
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