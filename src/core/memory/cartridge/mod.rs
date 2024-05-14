mod mbc;

use std::fs::File;
use std::path::PathBuf;
use std::io::{Read, Result, Write};
use log::warn;
use crate::core::memory::cartridge::mbc::{make_mbc, Mbc};

#[derive(Debug, Copy, Clone)]
pub enum CartridgeType {
    None = 0x00,
    MBC1 = 0x01,
    MBC1_RAM = 0x02,
    MBC1_RAM_BATTERY = 0x03,
    MBC5 = 0x19,
    MBC5_RAM = 0x1A,
    MBC5_RAM_BATTERY = 0x1B
}

impl From<u8> for CartridgeType {
    fn from(value: u8) -> Self {
        match value {
            0x00 => Self::None,
            0x01 => Self::MBC1,
            0x02 => Self::MBC1_RAM,
            0x03 => Self::MBC1_RAM_BATTERY,
            0x19 => Self::MBC5,
            0x1A => Self::MBC5_RAM,
            0x1B => Self::MBC5_RAM_BATTERY,
            _ => {
                warn!("Unknown cartridge type 0x{:x}. Falling back to None", value);
                Self::None
            }
        }
    }
}

struct CartridgeHeader {
    entry_point: Vec<u8>,
    logo: Vec<u8>,
    title: String,
    manufacturer_code: String,
    cgb: bool,
    new_licensee_code: String,
    sgb: bool,
    cart_type: CartridgeType,
    rom_size: usize,
    ram_size: usize,
    dest_code: u8,
    old_licensee_code: u8,
    version: u8,
    header_checksum: u8,
    global_checksum: Vec<u8>
}

impl CartridgeHeader {
    pub fn new(rom: &Vec<u8>) -> Self {
        let title = String::from_utf8(rom[0x134..=0x143].to_owned()).unwrap_or(String::from("<Invalid string>"));
        let manufacturer_code = String::from_utf8(rom[0x13F..=0x142].to_owned()).unwrap_or(String::from("<Invalid string>"));
        let new_licensee_code = String::from_utf8(rom[0x144..=0x145].to_owned()).unwrap_or(String::from("<Invalid string>"));
        let cart_type = CartridgeType::from(rom[0x147]);
        let rom_size: usize = match rom[0x148] as usize {
            val @ 0x00..=0x08 => 0x8000 * (1 << val),
            0x52 => 72 * 0x4000,
            0x53 => 80 * 0x4000,
            0x54 => 96 * 0x4000,
            val => {
                warn!("Unknown ROM size {:x}. Falling back to 32 KiB", val);
                0x8000
            }
        };
        let ram_size: usize = match rom[0x149] as usize {
            0x00 | 0x01 => 0,
            0x02 => 0x2000,
            0x03 => 4 * 0x2000,
            0x04 => 16 * 0x2000,
            0x05 => 8 * 0x2000,
            val => {
                warn!("Unknown RAM size 0x{:x}. Falling back to 0", val);
                0
            }
        };
        CartridgeHeader {
            entry_point: rom[0x100..=0x103].to_owned(),
            logo: rom[0x104..=0x133].to_owned(),
            title,
            manufacturer_code,
            cgb: rom[0x143] & 0x80 == 0x80,
            new_licensee_code,
            sgb: rom[0x146] == 0x03,
            cart_type,
            rom_size,
            ram_size,
            dest_code: rom[0x14A],
            old_licensee_code: rom[0x14B],
            version: rom[0x14C],
            header_checksum: rom[0x14D],
            global_checksum: rom[0x14E..=0x14F].to_owned()
        }
    }
}

pub struct Cartridge {
    rom_path: PathBuf,
    sav_path: PathBuf,
    header: CartridgeHeader,
    mbc: Box<dyn Mbc>
}

impl Cartridge {
    pub fn open(rom_path: PathBuf) -> Result<Self> {
        let mut rom = Vec::new();
        File::open(&rom_path)?
            .read_to_end(&mut rom)?;

        let sav_path = rom_path.with_extension("sav");
        let sav = match File::open(&sav_path) {
            Ok(mut f) => {
                let mut res = Vec::new();
                f.read_to_end(&mut res)?;
                Some(res)
            },
            Err(_) => None
        };
        let header = CartridgeHeader::new(&rom);
        if rom.len() != header.rom_size {
            warn!("Inconsistent ROM size. Cartridge header reports {:x}, but actual size is {:x}", header.rom_size, rom.len());
        }
        let mbc = make_mbc(&header, rom, sav);
        Ok(Cartridge { rom_path, sav_path, header, mbc })
    }

    pub fn read(&self, addr: u16) -> u8 {
        match addr {
            0x0000..=0x7FFF => self.mbc.read(addr),
            0xA000..=0xBFFF => self.mbc.read_ext_ram(addr),
            _ => {
                warn!("Reading from invalid cartridge address 0x{:x}. Returning 0xFF", addr);
                0xFF
            }
        }
    }

    pub fn write(&mut self, addr: u16, val: u8) {
        match addr {
            0x0000..=0x7FFF => self.mbc.write(addr, val),
            0xA000..=0xBFFF => self.mbc.write_ext_ram(addr, val),
            _ => warn!("Writing to invalid cartridge address 0x{:x} value {:x}", addr, val)
        }
    }
    
    pub fn save(&self) {
        if self.mbc.has_ram() && self.mbc.has_battery() {
            let mut f = File::create(&self.sav_path).expect("Failed to create save file");
            warn!("Writing save file to {:?}", self.sav_path);
            f.write_all(self.mbc.ram().unwrap()).expect("Failed to write save file");
        }
    }

    pub fn is_cgb(&self) -> bool {
        self.header.cgb
    }
}