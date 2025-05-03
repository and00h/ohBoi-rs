// Copyright Antonio Porsia 2025. Licensed under the EUPL-1.2 or later.

mod mbc;

use std::fs::File;
use std::path::PathBuf;
use std::io::{Read, Result, Write};
use std::ops::Index;
use std::slice::SliceIndex;
use log::warn;
use crate::memory::cartridge::mbc::{make_mbc, Mbc};

/// Represents the type of a Game Boy cartridge.
#[derive(Debug, Copy, Clone)]
pub enum CartridgeType {
    /// No cartridge.
    None = 0x00,
    /// Memory Bank Controller 1 (MBC1).
    MBC1 = 0x01,
    /// MBC1 with RAM.
    MBC1_RAM = 0x02,
    /// MBC1 with RAM and battery backup.
    MBC1_RAM_BATTERY = 0x03,
    /// Memory Bank Controller 3 (MBC3).
    /// This includes RTC support.
    MBC3_TIMER_BATTERY = 0x0F,
    /// MBC3 with RAM.
    /// This includes RTC support.
    MBC3_TIMER_RAM_BATTERY = 0x10,
    /// MBC3 without RAM and RTC.
    MBC3 = 0x11,
    /// MBC3 with RAM.
    /// Without RTC.
    MBC3_RAM = 0x12,
    /// MBC3 with RAM and battery backup.
    /// Without RTC.
    MBC3_RAM_BATTERY = 0x13,
    /// Memory Bank Controller 5 (MBC5).
    MBC5 = 0x19,
    /// MBC5 with RAM.
    MBC5_RAM = 0x1A,
    /// MBC5 with RAM and battery backup.
    MBC5_RAM_BATTERY = 0x1B,
}

impl From<u8> for CartridgeType {
    /// Converts a `u8` value into a `CartridgeType`.
    ///
    /// # Arguments
    ///
    /// * `value` - The `u8` value representing the cartridge type.
    ///
    /// # Returns
    ///
    /// The corresponding `CartridgeType`. Defaults to `None` for unknown values.
    fn from(value: u8) -> Self {
        match value {
            0x00 => Self::None,
            0x01 => Self::MBC1,
            0x02 => Self::MBC1_RAM,
            0x03 => Self::MBC1_RAM_BATTERY,
            0x0F => Self::MBC3_TIMER_BATTERY,
            0x10 => Self::MBC3_TIMER_RAM_BATTERY,
            0x11 => Self::MBC3,
            0x12 => Self::MBC3_RAM,
            0x13 => Self::MBC3_RAM_BATTERY,
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

/// Represents the header of a Game Boy cartridge.
///
/// The header contains metadata about the cartridge, such as its title, type, and memory sizes.
struct CartridgeHeader {
    /// The entry point of the cartridge.
    entry_point: Vec<u8>,
    /// The Nintendo logo data.
    logo: Vec<u8>,
    /// The title of the game.
    title: String,
    /// The manufacturer code.
    manufacturer_code: String,
    /// Indicates if the cartridge supports Color Game Boy (CGB).
    cgb: bool,
    /// The new licensee code.
    new_licensee_code: String,
    /// Indicates if the cartridge supports Super Game Boy (SGB).
    sgb: bool,
    /// The type of the cartridge.
    cart_type: CartridgeType,
    /// The size of the ROM in bytes.
    rom_size: usize,
    /// The size of the RAM in bytes.
    ram_size: usize,
    /// The destination code.
    dest_code: u8,
    /// The old licensee code.
    old_licensee_code: u8,
    /// The version of the cartridge.
    version: u8,
    /// The checksum of the header.
    header_checksum: u8,
    /// The global checksum of the cartridge.
    global_checksum: Vec<u8>,
}

impl CartridgeHeader {
    /// Creates a new `CartridgeHeader` from the given ROM data.
    ///
    /// # Arguments
    ///
    /// * `rom` - A reference to the ROM data as a `Vec<u8>`.
    ///
    /// # Returns
    ///
    /// A new `CartridgeHeader` instance.
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
            global_checksum: rom[0x14E..=0x14F].to_owned(),
        }
    }
}

/// Represents a Game Boy cartridge.
///
/// This structure manages the ROM, RAM, and memory bank controller (MBC) for the cartridge.
pub struct Cartridge {
    /// The path to the ROM file.
    rom_path: PathBuf,
    /// The path to the save file.
    sav_path: PathBuf,
    /// The header of the cartridge.
    header: CartridgeHeader,
    /// The memory bank controller (MBC) used by the cartridge.
    pub(crate) mbc: Box<dyn Mbc>,
}

impl Cartridge {
    /// Opens a cartridge from the given ROM file path.
    ///
    /// # Arguments
    ///
    /// * `rom_path` - The path to the ROM file.
    ///
    /// # Returns
    ///
    /// A `Result` containing the `Cartridge` instance or an error.
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
            Err(_) => None,
        };
        let header = CartridgeHeader::new(&rom);
        if rom.len() != header.rom_size {
            warn!("Inconsistent ROM size. Cartridge header reports {:x}, but actual size is {:x}", header.rom_size, rom.len());
        }
        let mbc = make_mbc(&header, rom, sav);
        Ok(Cartridge { rom_path, sav_path, header, mbc })
    }

    /// Reads a byte from the cartridge at the specified address.
    ///
    /// # Arguments
    ///
    /// * `addr` - The address to read from.
    ///
    /// # Returns
    ///
    /// The value at the specified address.
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

    /// Writes a byte to the cartridge at the specified address.
    ///
    /// # Arguments
    ///
    /// * `addr` - The address to write to.
    /// * `val` - The value to write.
    pub fn write(&mut self, addr: u16, val: u8) {
        match addr {
            0x0000..=0x7FFF => self.mbc.write(addr, val),
            0xA000..=0xBFFF => self.mbc.write_ext_ram(addr, val),
            _ => warn!("Writing to invalid cartridge address 0x{:x} value {:x}", addr, val),
        }
    }

    /// Saves the current state of the cartridge to the save file.
    pub fn save(&self) {
        if self.mbc.has_ram() && self.mbc.has_battery() {
            let mut f = File::create(&self.sav_path).expect("Failed to create save file");
            warn!("Writing save file to {:?}", self.sav_path);
            f.write_all(self.mbc.ram().unwrap()).expect("Failed to write save file");
        }
    }

    /// Checks if the cartridge supports Color Game Boy (CGB).
    ///
    /// # Returns
    ///
    /// `true` if the cartridge supports CGB, `false` otherwise.
    pub fn is_cgb(&self) -> bool {
        self.header.cgb
    }

    /// Returns a reference to the ROM data.
    ///
    /// This method is only available when the `debug_ui` feature is enabled.
    #[cfg(feature = "debugging")]
    pub fn rom(&self) -> &[u8] {
        &self.mbc.rom()
    }

    /// Returns a reference to the external RAM data, if available.
    ///
    /// This method is only available when the `debug_ui` feature is enabled.
    #[cfg(feature = "debugging")]
    pub fn ext_ram(&self) -> Option<&[u8]> {
        self.mbc.ram().map(|ram| &ram[..])
    }
}

impl<Idx> Index<Idx> for Cartridge
where
    Idx: SliceIndex<[u8]> {
    type Output = Idx::Output;

    /// Indexes into the ROM data of the cartridge.
    ///
    /// # Arguments
    ///
    /// * `index` - The index or range to access.
    ///
    /// # Returns
    ///
    /// A reference to the indexed data.
    fn index(&self, index: Idx) -> &Self::Output {
        &self.mbc.rom()[index]
    }
}