// Copyright Antonio Porsia 2025. Licensed under the EUPL-1.2 or later.

use super::Mbc;

pub(crate) struct None {
    rom: Vec<u8>
}

impl None {
    pub fn new(rom: Vec<u8>) -> Self {
        Self { rom }
    }
}

impl Mbc for None {
    #[inline]
    fn read(&self, addr: u16) -> u8 {
        self.rom[addr as usize]
    }
    
    #[inline]
    fn write(&mut self, _addr: u16, _val: u8) {}
    
    #[inline]
    fn rom(&self) -> &Vec<u8> {
        &self.rom
    }
}