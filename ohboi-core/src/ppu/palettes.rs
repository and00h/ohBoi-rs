// Copyright Antonio Porsia 2025. Licensed under the EUPL-1.2 or later.

use bitfield::bitfield;

const DMG_PALETTE: [[u8; 4]; 4] = [
    [0xFF, 0xFF, 0xFF, 0xFF],
    [0xCC, 0xCC, 0xCC, 0xFF],
    [0x77, 0x77, 0x77, 0xFF],
    [0x00, 0x00, 0x00, 0xFF]
];

pub struct DmgPalette {
    pub(super) value: u8,
    colors: [[u8; 4]; 4]
}

impl DmgPalette {
    pub fn new(value: u8) -> Self {
        let mut res = Self {
            value: 0,
            colors: [[0; 4]; 4]
        };

        res.update_palette(value);
        res
    }

    pub fn update_palette(&mut self, mut new_value: u8) {
        self.value = new_value;
        self.colors
            .iter_mut()
            .for_each(|color| {
                let new_color = &DMG_PALETTE[(new_value & 3) as usize];
                color.copy_from_slice(new_color);
                new_value >>= 2;
            })
    }

    pub fn colors(&self) -> &[[u8; 4]; 4] {
        &self.colors
    }

    pub fn value(&self) -> u8 {
        self.value
    }
}

bitfield! {
    #[derive(Copy, Clone, Default)]
    pub struct CgbPaletteColor(u16);
    impl Debug;
    pub r, _: 4, 0;
    pub g, _: 9, 5;
    pub b, _: 14, 10;
    pub unused, _: 15;
    pub lo, set_lo: 7, 0;
    pub hi, set_hi: 15, 8;
}

bitfield! {
    #[derive(Copy, Clone, Default)]
    pub struct CgbPaletteIndex(u8);
    impl Debug;
    pub index, set_index: 5, 0;
    pub palette, _: 5, 3;
    pub color, _: 2, 1;
    pub hilo, _: 0;
    pub auto_increment, _: 7;
}

#[derive(Debug, Default)]
pub struct CgbPalette {
    pub(super) data: [[CgbPaletteColor; 4]; 8],
    pub(super) index: CgbPaletteIndex
}

impl CgbPalette {
    pub fn new() -> Self {
        Self {
            data: [[CgbPaletteColor(0); 4]; 8],
            index: CgbPaletteIndex(0)
        }
    }

    pub fn write_data(&mut self, val: u8) {
        let palette = self.index.palette() as usize;
        let color = self.index.color() as usize;
        let color_data = self.index.hilo();
        if color_data {
            self.data[palette][color].set_hi(val as u16);
        } else {
            self.data[palette][color].set_lo(val as u16);   
        }
        if self.index.auto_increment() {
            self.increment_index();
        }
    }

    pub fn write_index(&mut self, val: u8) {
        self.index = CgbPaletteIndex(val);
    }

    pub fn read_data(&self) -> u8 {
        let palette = self.index.palette() as usize;
        let color = self.index.color() as usize;
        let color_data = self.index.hilo();
        if color_data {
            self.data[palette][color].hi() as u8
        } else {
            self.data[palette][color].lo() as u8
        }
    }

    pub fn read_index(&self) -> u8 {
        self.index.0
    }

    pub fn increment_index(&mut self) {
        self.index.set_index((self.index.index() + 1) % 64);
    }

    pub fn color(&self) -> u32 {
        let palette = self.index.palette() as usize;
        let color = self.index.color() as usize;
        let r = (self.data[palette][color].r() << 3) | (self.data[palette][color].r() >> 2);
        let g = (self.data[palette][color].g() << 3) | (self.data[palette][color].g() >> 2);
        let b = (self.data[palette][color].b() << 3) | (self.data[palette][color].b() >> 2);
        
        0x000000FF | ((r as u32) << 24) | ((g as u32) << 16) | ((b as u32) << 8)
    }
    
    pub fn color_array(&self, palette_index: usize) -> [[u8; 4]; 4] {
        let mut res = [[0; 4]; 4];
        for i in 0..4 {
            let r = ((self.data[palette_index][i].r() << 3) | (self.data[palette_index][i].r() >> 2)) as u8;
            let g = ((self.data[palette_index][i].g() << 3) | (self.data[palette_index][i].g() >> 2)) as u8;
            let b = ((self.data[palette_index][i].b() << 3) | (self.data[palette_index][i].b() >> 2)) as u8;
            res[i] = [r, g, b, 0xFF];
        }
        
        res
    }
}