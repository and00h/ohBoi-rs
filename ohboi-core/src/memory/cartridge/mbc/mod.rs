// Copyright Antonio Porsia 2025. Licensed under the EUPL-1.2 or later.

mod none;
mod mbc1;
mod mbc3;
mod mbc5;

use log::warn;
use crate::memory::cartridge::{CartridgeHeader, CartridgeType};

pub(crate) use none::None;
pub(crate) use mbc1::Mbc1;
pub(crate) use mbc3::Mbc3;
pub(crate) use mbc5::Mbc5;

#[allow(clippy::upper_case_acronyms)]
enum BankingMode {
    ROM,
    RAM
}

const ROM_BANK_SIZE: usize = 0x4000;
const RAM_BANK_SIZE: usize = 0x2000;

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
    #[allow(unreachable_patterns)] 
    match header.cart_type {
        CartridgeType::None => Box::new(None::new(rom)),
        CartridgeType::Mbc1 =>
            Box::new(Mbc1::new(rom, header, None, false)),
        CartridgeType::Mbc1Ram =>
            Box::new(Mbc1::new(rom, header, ram, false)),
        CartridgeType::Mbc1RamBattery => Box::new(Mbc1::new(rom, header, ram, true)),
        CartridgeType::Mbc3TimerBattery =>
            Box::new(Mbc3::new(rom, header, None, true, true)),
        CartridgeType::Mbc3TimerRamBattery =>
            Box::new(Mbc3::new(rom, header, ram, true, true)),
        CartridgeType::Mbc3 =>
            Box::new(Mbc3::new(rom, header, None, false, false)),
        CartridgeType::Mbc3Ram =>
            Box::new(Mbc3::new(rom, header, ram, false, false)),
        CartridgeType::Mbc3RamBattery =>
            Box::new(Mbc3::new(rom, header, ram, true, false)),
        CartridgeType::Mbc5 =>
            Box::new(Mbc5::new(rom, header, None, false)),
        CartridgeType::Mbc5Ram =>
            Box::new(Mbc5::new(rom, header, ram, false)),
        CartridgeType::Mbc5RamBattery => Box::new(Mbc5::new(rom, header, ram, true)),
        t => {
            warn!("Unimplemented cartridge type {:?}. Falling back to None", t);
            Box::new(None::new(rom))
        }
    }
}