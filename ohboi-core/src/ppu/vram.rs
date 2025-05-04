// Copyright Antonio Porsia 2025. Licensed under the EUPL-1.2 or later.

use std::ops::{Index, IndexMut};
use std::slice::SliceIndex;
use bitfield::bitfield;

macro_rules! tileid_to_address {
    ($tileid:expr, $signed:expr, $bank:expr) => {
        if !$signed {
            ($tileid as usize) * 16 + $bank * 0x2000
        } else {
            $bank * 0x2000 + ((($tileid as i8) as i32 + 256) * 16) as usize
        }
    };
}

macro_rules! get_tile_pixel {
    ($line_data:expr, $x:expr) => {
        (($line_data[1] >> $x) & 1) << 1 | (($line_data[0] >> $x) & 1)
    };
}

#[derive(Default, Copy, Clone)]
pub struct Tile {
    colors: [[u8; 8]; 8],
}

impl Index<usize> for Tile {
    type Output = [u8; 8];

    fn index(&self, index: usize) -> &Self::Output {
        &self.colors[index]
    }
}

impl IndexMut<usize> for Tile {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.colors[index]
    }
}

impl From<&[u8]> for Tile {
    fn from(data: &[u8]) -> Self {
        let mut tile = Tile::default();
        for y in 0..8 {
            let line = &data[y * 2..y * 2 + 2];
            for x in 0..8 {
                tile[y][x] = get_tile_pixel!(line, x);
            }
        }
        tile
    }
}

bitfield! {
    #[derive(Copy, Clone, Default)]
    pub struct TileAttributes(u8);
    impl Debug;
    pub palette, _: 2, 0;
    pub bank, _: 3, 3;
    pub x_flip, _: 5;
    pub y_flip, _: 6;
    pub priority, _: 7;
}

/// Represents a Video RAM (VRAM) structure for a Game Boy emulator.
/// This structure manages the VRAM memory and allows reading, writing, 
/// and switching between VRAM banks.
pub struct Vram {
    /// The VRAM memory, consisting of 16 KB (0x4000 bytes).
    vram: [u8; 0x4000],
    /// The currently active VRAM bank (0 or 1).
    vram_bank: usize,
    cgb: bool,
}

impl Vram {
    /// Creates a new `Vram` instance with initialized memory and bank.
    ///
    /// # Returns
    /// A new `Vram` instance with all memory set to 0 and the bank set to 0.
    pub fn new(cgb: bool) -> Self {
        Self {
            vram: [0; 0x4000],
            vram_bank: 0,
            cgb,
        }
    }

    /// Reads a byte from the VRAM at the specified address.
    ///
    /// # Parameters
    /// - `addr`: The address to read from (0x0000 to 0x1FFF).
    ///
    /// # Returns
    /// The byte value stored at the specified address in the current VRAM bank.
    pub fn read(&self, addr: u16) -> u8 {
        self.vram[self.vram_bank * 0x2000 + addr as usize]
    }

    /// Writes a byte to the VRAM at the specified address.
    ///
    /// # Parameters
    /// - `addr`: The address to write to (0x0000 to 0x1FFF).
    /// - `val`: The byte value to write to the specified address.
    pub fn write(&mut self, addr: u16, val: u8) {
        self.vram[self.vram_bank * 0x2000 + addr as usize] = val;
    }

    /// Sets the active VRAM bank.
    ///
    /// # Parameters
    /// - `bank`: The VRAM bank to activate (0 or 1).
    pub fn set_vram_bank(&mut self, bank: usize) {
        if self.cgb {
            self.vram_bank = bank;
        }
    }
    
    pub fn vram_bank(&self) -> usize {
        self.vram_bank
    }
    
    pub fn get_tile(&self, tile_map_index: u16, signed: bool) -> (Tile, TileAttributes) {
        let attr = if self.cgb { TileAttributes(self.vram[tile_map_index as usize + 0x2000]) } else { TileAttributes(0) };
        let tile_id = self.vram[tile_map_index as usize];
        let tile_address = tileid_to_address!(tile_id, signed, attr.bank() as usize);
        let tile_data = &self.vram[tile_address..tile_address + 16];
        let tile = Tile::from(tile_data);
        (tile, attr)
    }
    
    pub fn get_sprite_tile(&self, sprite_tile_location: u8, bank: usize) -> Tile {
        let tile_address = tileid_to_address!(sprite_tile_location, false, bank);
        let tile_data = &self.vram[tile_address..tile_address + 16];
        Tile::from(tile_data)
    }
    
    
    pub fn reset(&mut self) {
        self.vram = [0; 0x4000];
        self.vram_bank = 0;
    }
}

impl<Idx> Index<Idx> for Vram
where
    Idx: SliceIndex<[u8]> {
    type Output = Idx::Output;

    fn index(&self, index: Idx) -> &Self::Output {
        &self.vram[index]
    }
}

impl<Idx> IndexMut<Idx> for Vram
where
    Idx: SliceIndex<[u8]> {
    fn index_mut(&mut self, index: Idx) -> &mut Self::Output {
        &mut self.vram[index]
    }
}