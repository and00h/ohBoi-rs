use log::warn;
use crate::core::memory::cartridge::{Cartridge, CartridgeHeader, CartridgeType};

pub(super) mod none;
pub(super) mod mbc1;
pub(super) mod mbc3;
pub(super) mod mbc5;

enum BankingMode {
    ROM,
    RAM
}

const RAM_SIZE_MAP: [usize; 6] = [
    0, 2048, 8192, 32768, 131072, 65535
];

const ROM_BANK_SIZE: usize = 0x4000;
const RAM_BANK_SIZE: usize = 0x2000;

fn calc_rom_size(header_rom_size: usize) -> usize {
    match header_rom_size {
        0..=8 => 0x8000 << header_rom_size,
        0x52 => 72 * ROM_BANK_SIZE,
        0x53 => 80 * ROM_BANK_SIZE,
        0x54 => 96 * ROM_BANK_SIZE,
        _ => panic!("Invalid ROM size in cartridge header: {:02X}", header_rom_size)
    }
}

pub(crate) trait Mbc {
    fn read(&self, addr: u16) -> u8;
    fn write(&mut self, addr: u16, val: u8);
    fn read_ext_ram(&self, _addr: u16) -> u8 { 0xFF }
    fn write_ext_ram(&mut self, _addr: u16, _val: u8) {}

    fn num_banks(&self) -> usize { 2 }
    fn num_ram_banks(&self) -> usize { 0 }

    fn has_battery(&self) -> bool { false }
    fn has_rtc(&self) -> bool { false }
    fn has_ram(&self) -> bool { false }

    fn ram(&self) -> Option<&Vec<u8>> { None }
    fn rom(&self) -> &Vec<u8>;
}

pub(super) fn make_mbc(header: &CartridgeHeader, rom: Vec<u8>, ram: Option<Vec<u8>>) -> Box<dyn Mbc> {
    match header.cart_type {
        CartridgeType::None => Box::new(none::None::new(rom)),
        CartridgeType::MBC1 =>
            Box::new(mbc1::Mbc1::new(rom, header, None, false)),
        CartridgeType::MBC1_RAM =>
            Box::new(mbc1::Mbc1::new(rom, header, ram, false)),
        CartridgeType::MBC1_RAM_BATTERY => Box::new(mbc1::Mbc1::new(rom, header, ram, true)),
        CartridgeType::MBC3_TIMER_BATTERY =>
            Box::new(mbc3::Mbc3::new(rom, header, None, true, true)),
        CartridgeType::MBC3_TIMER_RAM_BATTERY =>
            Box::new(mbc3::Mbc3::new(rom, header, ram, true, true)),
        CartridgeType::MBC3 =>
            Box::new(mbc3::Mbc3::new(rom, header, None, false, false)),
        CartridgeType::MBC3_RAM =>
            Box::new(mbc3::Mbc3::new(rom, header, ram, false, false)),
        CartridgeType::MBC3_RAM_BATTERY =>
            Box::new(mbc3::Mbc3::new(rom, header, ram, true, false)),
        CartridgeType::MBC5 =>
            Box::new(mbc5::Mbc5::new(rom, header, None, false)),
        CartridgeType::MBC5_RAM =>
            Box::new(mbc5::Mbc5::new(rom, header, ram, false)),
        CartridgeType::MBC5_RAM_BATTERY => Box::new(mbc5::Mbc5::new(rom, header, ram, true)),
        t => {
            warn!("Unimplemented cartridge type {:?}. Falling back to None", t);
            Box::new(none::None::new(rom))
        }
    }
}