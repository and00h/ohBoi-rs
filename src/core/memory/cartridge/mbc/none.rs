use super::Mbc;

pub(super) struct None {
    rom: Vec<u8>
}

impl None {
    pub fn new(rom: Vec<u8>) -> Self {
        Self { rom }
    }
}

impl Mbc for None {
    fn read(&self, addr: u16) -> u8 {
        self.rom[addr as usize]
    }

    fn write(&mut self, addr: u16, val: u8) {
        self.rom[addr as usize] = val;
    }

    fn rom(&self) -> &Vec<u8> {
        &self.rom
    }
}