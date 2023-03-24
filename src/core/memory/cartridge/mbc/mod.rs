use log::warn;
use crate::core::memory::cartridge::{Cartridge, CartridgeHeader, CartridgeType};

pub(super) mod none;

pub(super) trait Mbc {
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
        t => {
            warn!("Unimplemented cartridge type {:?}. Falling back to None", t);
            Box::new(none::None::new(rom))
        }
    }
}