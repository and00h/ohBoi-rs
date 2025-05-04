// Copyright Antonio Porsia 2025. Licensed under the EUPL-1.2 or later.

use std::ops::{Index, IndexMut};
use bitfield::bitfield;

#[derive(Copy, Clone, Default)]
pub struct Sprite {
    pub oam_offset: u8,
    pub y: u8,
    pub x: u8,
    pub tile_location: u8,
    pub attributes: SpriteAttributes
}

impl Sprite {
    pub fn new(oam_offset: u8, oam_data: &[u8]) -> Self {
        Self {
            oam_offset,
            y: oam_data[0],
            x: oam_data[1],
            tile_location: oam_data[2],
            attributes: SpriteAttributes(oam_data[3])
        }
    }

    pub fn cgb_palette_number(&self) -> u8 {
        self.attributes.cgb_palette_number()
    }
    pub fn x_flip(&self) -> bool {
        self.attributes.x_flip()
    }
    pub fn y_flip(&self) -> bool {
        self.attributes.y_flip()
    }
    pub fn _vram_bank(&self) -> u8 {
        self.attributes.vram_bank() as u8
    }
    pub fn dmg_palette_number(&self) -> u8 {
        self.attributes.dmg_palette_number() as u8
    }
    pub fn has_priority(&self) -> bool {
        !self.attributes.bg_window_priority()
    }
}

bitfield! {
    #[derive(Copy, Clone, Default)]
    pub struct SpriteAttributes(u8);
    impl Debug;
    pub cgb_palette_number, set_cgb_palette_number: 2, 0;
    pub vram_bank, set_vram_bank: 3, 3;
    pub dmg_palette_number, set_dmg_palette_number: 4;
    pub x_flip, set_x_flip: 5;
    pub y_flip, set_y_flip: 6;
    pub bg_window_priority, set_bg_window_priority: 7;
}

pub struct Oam {
    oam: [u8; 0xA0],
}

impl Oam {
    pub fn new() -> Self {
        Self {
            oam: [0; 0xA0],
        }
    }
    
    pub fn reset(&mut self) {
        self.oam.fill(0);
    }
    
    #[inline]
    pub fn read(&self, addr: u16) -> u8 {
        self.oam[addr as usize]
    }
    
    #[inline]
    pub fn write(&mut self, addr: u16, value: u8) {
        self.oam[addr as usize] = value;
    }
    
    #[inline]
    pub fn get_sprite(&self, index: usize) -> Sprite {
        let sprite_data = &self.oam[index * 4..index * 4 + 4];
        Sprite::new(index as u8, sprite_data)
    }
}

impl Index<usize> for Oam {
    type Output = u8;

    fn index(&self, index: usize) -> &Self::Output {
        &self.oam[index]
    }
}

impl IndexMut<usize> for Oam {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.oam[index]
    }
}