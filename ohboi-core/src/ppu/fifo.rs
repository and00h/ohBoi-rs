// Copyright Antonio Porsia 2025. Licensed under the EUPL-1.2 or later.

use std::collections::VecDeque;
use crate::ppu::vram::{get_tile_pixel, tileidx_to_address, Tile, TileAttributes, Vram};
use crate::ppu::oam::{Sprite, SpriteAttributes};
use crate::ppu::Ppu;

enum PixelFetcherState {
    GetTile,
    GetTileDataLo,
    GetTileDataHi,
    Sleep,
    Push
}

enum FetcherState {
    GetTile,
    GetTileDataLo,
    GetTileDataHi,
    Push
}

struct TileData {
    pub tile_row_index: u16,
    pub tile_row_addr: u16,
    pub tile_index: i32,
    pub tile_y: u8,
    tile: Tile,
    tile_attributes: TileAttributes
}

impl TileData {
    pub fn reset(&mut self) {
        self.tile_row_index = 0;
        self.tile_row_addr = 0;
        self.tile_index = 0;
        self.tile_y = 0;
        self.tile_attributes = TileAttributes::default();
    }

    pub fn start_fetch(&mut self, x: u8, y: u8, tile_map: u16) {
        self.tile_index = 0;
        self.tile_y = y & 7;
        self.tile_row_index = (x >> 3) as u16;
        self.tile_row_addr = tile_map + (((y as u16) >> 3) << 5);
    }

    pub fn update_tile_index(&mut self, vram: &Vram, signed_tileset: bool) {
        (self.tile, self.tile_attributes) = 
            vram.get_tile(self.tile_row_addr + self.tile_row_index, signed_tileset);
    }

    pub fn get_tile_pixel(&self, cgb: bool, x: usize) -> TilePixel {
        if !cgb {
            TilePixel {
                color: self.tile[self.tile_y as usize][x],
                palette: 0,
                priority: false,
            }
        } else {
            let x = if self.tile_attributes.x_flip() { 7 - x } else { x };
            let y = if self.tile_attributes.y_flip() { 7 - self.tile_y } else { self.tile_y } as usize;
            TilePixel {
                color: self.tile[y][x],
                palette: self.tile_attributes.palette(),
                priority: self.tile_attributes.priority(),
            }
        }
    }

    #[inline]
    pub fn increment_row_index(&mut self) {
        self.tile_row_index = (self.tile_row_index + 1) & 0x1F;
    }
}

struct SpriteData {
    pub current_sprite: Sprite,
    pub sprite_tile_index: u8,
    pub sprite_tile_y: u8
}

impl SpriteData {
    pub fn reset(&mut self) {
        self.sprite_tile_index = 0;
        self.sprite_tile_y = 0;
        self.current_sprite = Sprite::default();
    }

    pub fn start_fetch(&mut self, y: u8, sprite: &Sprite, use_8x16: bool) {
        self.sprite_tile_index = sprite.tile_location;
        if use_8x16 {
            if y >= sprite.y.wrapping_sub(8) {
                self.sprite_tile_index |= 1;
            } else {
                self.sprite_tile_index &= 0xFE;
            }
        }
        self.sprite_tile_y = (y.wrapping_sub(sprite.y)) & 0b00000111;
        if sprite.y_flip() {
            self.sprite_tile_y = 7 - self.sprite_tile_y;
            if use_8x16 {
                if y >= sprite.y.wrapping_sub(8) {
                    self.sprite_tile_index &= 0xFE;
                } else {
                    self.sprite_tile_index |= 1
                }
            }
        }
        self.current_sprite = *sprite;
    }

    pub fn get_sprite_pixel(&self, tile_x: usize, cgb: bool, vram: &Vram) -> SpritePixel {
        if !cgb {
            let sprite_y = self.sprite_tile_y as usize;
            let sprite_x = if self.current_sprite.x_flip() { tile_x } else { 7 - tile_x };
            let tile = vram.get_sprite_tile(self.sprite_tile_index, self.current_sprite.attributes.vram_bank() as usize);
            SpritePixel {
                pixel: TilePixel {
                    color: tile[sprite_y][sprite_x],
                    palette: self.current_sprite.dmg_palette_number(),
                    priority: self.current_sprite.has_priority(),
                },
                oam_offset: self.current_sprite.oam_offset,
            }
        } else {
            let x = if self.current_sprite.x_flip() { tile_x } else { 7 - tile_x };
            let y = self.sprite_tile_y as usize;
            let bank = self.current_sprite.attributes.vram_bank() as usize;
            let tile = vram.get_sprite_tile(self.sprite_tile_index, bank);
            let color = tile[y][x];
            SpritePixel {
                pixel: TilePixel {
                    color,
                    palette: self.current_sprite.cgb_palette_number(),
                    priority: self.current_sprite.has_priority(),
                },
                oam_offset: self.current_sprite.oam_offset,
            }
        }
    }

    #[inline]
    pub fn has_priority_over_sprite(&self, oam_offset: u8) -> bool {
        self.current_sprite.oam_offset < oam_offset
    }
}

pub struct BackgroundFetcher {
    state: FetcherState,
    pub(super) fifo: VecDeque<TilePixel>,
    fetcher_x: u8,
    fetcher_y: u8,
    x_scroll: u8,
    tile: [u8; 2],
    tile_number: u8,
    tile_address: usize,
    tile_attributes: TileAttributes,
    paused: bool,
    clock_divider: bool,
    dummy_fetch: bool,
}

impl BackgroundFetcher {
    pub fn new() -> Self {
        Self {
            state: FetcherState::GetTile,
            fifo: VecDeque::new(),
            fetcher_x: 0,
            fetcher_y: 0,
            x_scroll: 0,
            tile: [0; 2],
            tile_number: 0,
            tile_address: 0,
            tile_attributes: TileAttributes::default(),
            paused: true,
            clock_divider: true,
            dummy_fetch: true,
        }
    }

    pub fn reset(&mut self) {
        self.state = FetcherState::GetTile;
        self.fifo.clear();
        self.fetcher_x = 0;
        self.fetcher_y = 0;
        self.x_scroll = 0;
        self.tile = [0; 2];
        self.tile_number = 0;
        self.tile_address = 0;
        self.tile_attributes = TileAttributes::default();
        self.paused = true;
        self.clock_divider = true;
        self.dummy_fetch = true;
    }
    
    pub fn clock(&mut self, ppu: &Ppu) {
        if self.paused {
            return;
        }
        
        self.clock_divider = !self.clock_divider;
        
        match self.state {
            FetcherState::GetTile if self.clock_divider => self.get_tile(ppu),
            FetcherState::GetTileDataLo if self.clock_divider => self.get_tile_data_lo(ppu),
            FetcherState::GetTileDataHi if self.clock_divider => self.get_tile_data_hi(ppu),
            FetcherState::Push if self.clock_divider => self.push(ppu),
            _ => {}
        }
    }
    
    pub fn start_fetch(&mut self, ppu: &Ppu) {
        self.fetcher_x = if ppu.window.rendering { 0 } else { ppu.scroll_x };
        self.fetcher_y = if ppu.window.rendering {
            ppu.window.internal_line_counter
        } else {
            (ppu.ly as u8).wrapping_add(ppu.scroll_y) 
        };
        self.x_scroll = ppu.scroll_x & 7;
        self.paused = false;
        self.state = FetcherState::GetTile;
        self.fifo.clear();
    }
    
    fn get_tile(&mut self, ppu: &Ppu) {
        let tile_x = (self.fetcher_x >> 3) as usize;
        let tile_y = (self.fetcher_y >> 3) as usize;
        let tilemap = if ppu.window.rendering {
            if ppu.lcdc.window_tile_map() {
                0x1C00
            } else {
                0x1800
            }
        } else if ppu.lcdc.bg_tile_map() {
            0x1C00
        } else {
            0x1800
        };
        self.tile_number = ppu.vram[tilemap + ((32 * tile_y + tile_x) & 0x3ff)];
        self.tile_attributes = 
            TileAttributes(ppu.vram[tilemap + ((32 * tile_y + tile_x) & 0x3ff) + 0x2000]);

        let signed_tileset = !ppu.lcdc.bg_window_tile_data();
        self.tile_address =
            tileidx_to_address!(self.tile_number as usize, 
                signed_tileset, 
                self.tile_attributes.bank() as usize);
        self.state = FetcherState::GetTileDataLo;
    }
    
    fn get_tile_data_lo(&mut self, ppu: &Ppu) {
        let mut y = self.fetcher_y as usize & 7;
        if self.tile_attributes.y_flip() { y = 7 - y; }
        self.tile[0] = 
            ppu.vram[self.tile_address + y * 2];
        self.state = FetcherState::GetTileDataHi;
    }
    
    fn get_tile_data_hi(&mut self, ppu: &Ppu) {
        let mut y = self.fetcher_y as usize & 7;
        if self.tile_attributes.y_flip() { y = 7 - y; }
        self.tile[1] = 
            ppu.vram[self.tile_address + y * 2 + 1];
        if self.fetcher_x == 0 && self.dummy_fetch {
            self.state = FetcherState::GetTile;
            self.dummy_fetch = false;
        } else {
            self.state = FetcherState::Push;
        }
    }
    
    fn push(&mut self, ppu: &Ppu) {
        if !self.fifo.is_empty() {
            return;
        }
        for x in (0..8).rev() {
            let tile_x = if self.tile_attributes.x_flip() {
                7 - x
            } else {
                x
            } as usize;
            let color = get_tile_pixel!(self.tile, tile_x);
            let pixel = TilePixel {
                color,
                palette: self.tile_attributes.palette(),
                priority: self.tile_attributes.priority(),
            };
            self.fifo.push_back(pixel);
            self.fetcher_x += 1;
        }
        self.state = FetcherState::GetTile;
    }
    
    pub fn end_scanline(&mut self) {
        self.paused = true;
        self.state = FetcherState::GetTile;
        self.fifo.clear();
        self.fetcher_x = 0;
        self.fetcher_y = 0;
        self.tile = [0; 2];
        self.dummy_fetch = true;
    }
    
    pub fn pause(&mut self) {
        self.paused = true;
        self.state = FetcherState::GetTile;
    }
    
    pub fn resume(&mut self) {
        self.paused = false;
    }
}

pub struct SpriteFetcher {
    state: FetcherState,
    pub(super) fifo: VecDeque<SpritePixel>,
    fetcher_y: u8,
    tile: [u8; 2],
    tile_number: u8,
    tile_address: usize,
    sprite: Sprite,
    paused: bool,
    clock_divider: bool,
}

impl SpriteFetcher {
    pub fn new() -> Self {
        Self {
            state: FetcherState::GetTile,
            fifo: VecDeque::new(),
            fetcher_y: 0,
            tile: [0; 2],
            tile_number: 0,
            tile_address: 0,
            sprite: Sprite::default(),
            paused: true,
            clock_divider: true,
        }
    }

    pub fn reset(&mut self) {
        self.state = FetcherState::GetTile;
        self.fifo.clear();
        self.fetcher_y = 0;
        self.tile = [0; 2];
        self.tile_number = 0;
        self.tile_address = 0;
        self.sprite = Sprite::default();
        self.paused = true;
        self.clock_divider = true;
    }

    pub fn clock(&mut self, ppu: &Ppu) {
        if self.paused {
            return;
        }
        
        self.clock_divider = !self.clock_divider;

        match self.state {
            FetcherState::GetTile if self.clock_divider => self.get_tile(ppu),
            FetcherState::GetTileDataLo if self.clock_divider => self.get_tile_data_lo(ppu),
            FetcherState::GetTileDataHi if self.clock_divider => self.get_tile_data_hi(ppu),
            FetcherState::Push => self.push(ppu),
            _ => {}
        }
    }

    pub fn start_fetch(&mut self, ppu: &Ppu, sprite_index: usize) {
        self.sprite = ppu.sprites[sprite_index];
        self.fetcher_y = ppu.ly as u8;
        self.paused = false;
        self.state = FetcherState::GetTile;
        self.fifo.clear();
    }

    fn get_tile(&mut self, ppu: &Ppu) {
        self.tile_number = self.sprite.tile_location;
        if ppu.lcdc.obj_size() {
            if self.fetcher_y >= self.sprite.y.wrapping_sub(8) {
                self.tile_number |= 1;
            } else {
                self.tile_number &= 0xFE;
            }
        }
        if self.sprite.y_flip() && ppu.lcdc.obj_size() {
            if self.fetcher_y >= self.sprite.y.wrapping_sub(8) {
                self.tile_number &= 0xFE;
            } else {
                self.tile_number |= 1;
            }
        }
        self.tile_address =
            tileidx_to_address!(self.tile_number as usize, 
                false, 
                self.sprite.attributes.vram_bank() as usize);
        self.state = FetcherState::GetTileDataLo;
    }

    fn get_tile_data_lo(&mut self, ppu: &Ppu) {
        let mut y = self.fetcher_y.wrapping_sub(self.sprite.y) as usize & 7;
        if self.sprite.y_flip() { y = 7 - y; }
        self.tile[0] =
            ppu.vram[self.tile_address + y * 2];
        self.state = FetcherState::GetTileDataHi;
    }

    fn get_tile_data_hi(&mut self, ppu: &Ppu) {
        let mut y = self.fetcher_y.wrapping_sub(self.sprite.y) as usize & 7;
        if self.sprite.y_flip() { y = 7 - y; }
        self.tile[1] =
            ppu.vram[self.tile_address + y * 2 + 1];
        self.state = FetcherState::Push;
    }

    fn push(&mut self, ppu: &Ppu) {
        for x in 0..8 {
            let tile_x = if self.sprite.x_flip() {
                x
            } else {
                7 - x
            } as usize;
            let color = get_tile_pixel!(self.tile, tile_x);
            let tile_pixel = TilePixel {
                color,
                palette: if ppu.cgb { self.sprite.cgb_palette_number() } else { self.sprite.dmg_palette_number() },
                priority: self.sprite.attributes.bg_window_priority(),
            };
            let sprite_pixel = SpritePixel {
                pixel: tile_pixel,
                oam_offset: self.sprite.oam_offset,
            };
            if self.fifo.len() <= x as usize {
                self.fifo.push_back(sprite_pixel);
            } else if self.fifo[x as usize].pixel.color == 0 ||
                (ppu.cgb && self.sprite.oam_offset < self.fifo[x as usize].oam_offset)
            {
                self.fifo[x as usize] = sprite_pixel;
            }
        }
        self.paused = true;
        self.state = FetcherState::GetTile;
    }
    
    pub fn is_fetching(&self) -> bool {
        !self.paused
    }
    pub fn end_scanline(&mut self) {
        self.paused = true;
        self.state = FetcherState::GetTile;
        self.fifo.clear();
    }
}

pub struct PixelFetcher {
    state: PixelFetcherState,
    tile_data: TileData,
    sprite_data: SpriteData,

    scroll_quantity: u8,

    pub(crate) rendering_sprites: bool,

    dot_clock_divider: bool,
    bg_fifo: VecDeque<TilePixel>,
    spr_fifo: VecDeque<SpritePixel>,
}

impl PixelFetcher {
    pub fn new() -> Self {
        let sprite_data = SpriteData {
            current_sprite: Sprite::default(),
            sprite_tile_index: 0,
            sprite_tile_y: 0,
        };

        let tile_data = TileData {
            tile_row_index: 0,
            tile_row_addr: 0,
            tile_index: 0,
            tile_y: 0,
            tile: Default::default(),
            tile_attributes: TileAttributes::default(),
        };
        Self {
            state: PixelFetcherState::GetTile,
            tile_data,
            sprite_data,
            scroll_quantity: 0,
            rendering_sprites: false,
            dot_clock_divider: false,
            bg_fifo: VecDeque::new(),
            spr_fifo: VecDeque::new(),
        }
    }

    pub fn reset(&mut self) {
        self.state = PixelFetcherState::GetTile;
        self.tile_data.reset();
        self.sprite_data.reset();
        self.scroll_quantity = 0;
        self.rendering_sprites = false;
        self.dot_clock_divider = false;
        self.bg_fifo.clear();
        self.spr_fifo.clear();
    }

    pub fn start(&mut self, x: u8, y: u8, tile_map: u16, scroll: u8) {
        self.state = PixelFetcherState::GetTile;
        self.dot_clock_divider = false;
        self.scroll_quantity = scroll;
        self.rendering_sprites = false;

        self.tile_data.start_fetch(x, y, tile_map);

        self.bg_fifo.clear();
    }

    pub fn start_sprite_fetch(&mut self, sprite: Sprite, use_8x16: bool, y: u8) {
        self.state = PixelFetcherState::GetTile;
        self.dot_clock_divider = false;
        self.rendering_sprites = true;
        self.sprite_data.start_fetch(y, &sprite, use_8x16);
    }

    pub fn step(&mut self, vram: &Vram, cgb: bool, signed_tileset: bool) {
        match self.state {
            PixelFetcherState::GetTile => self.get_tile(vram, signed_tileset, cgb),
            PixelFetcherState::GetTileDataLo => self.get_tile_data_lo(),
            PixelFetcherState::GetTileDataHi => self.get_tile_data_hi(),
            PixelFetcherState::Sleep => self.sleep(),
            PixelFetcherState::Push => self.push(cgb, vram)
        }
    }

    fn step_dot_clock(&mut self) {
        self.dot_clock_divider = !self.dot_clock_divider;
    }

    fn get_tile(&mut self, vram: &Vram, signed_tileset: bool, cgb: bool) {
        self.step_dot_clock();
        if self.dot_clock_divider {
            if !self.rendering_sprites {
                self.tile_data.update_tile_index(vram, signed_tileset);
            }
            self.state = PixelFetcherState::GetTileDataLo;
        }
    }

    fn get_tile_data_lo(&mut self) {
        self.step_dot_clock();
        if self.dot_clock_divider {
            self.state = PixelFetcherState::GetTileDataHi;
        }
    }

    fn get_tile_data_hi(&mut self) {
        self.step_dot_clock();
        if self.dot_clock_divider {
            self.state = if self.rendering_sprites {
                if self.spr_fifo.len() <= 8 {
                    PixelFetcherState::Push
                } else {
                    PixelFetcherState::Sleep
                }
            } else if self.bg_fifo.len() <= 8 {
                PixelFetcherState::Push
            } else {
                PixelFetcherState::Sleep
            }
        }
    }

    fn sleep(&mut self) {
        self.step_dot_clock();
        if self.dot_clock_divider {
            self.state = PixelFetcherState::Push;
        }
    }

    fn push(&mut self, cgb: bool, vram: &Vram) {
        if self.rendering_sprites {
            if self.spr_fifo.len() <= 8 {
                for tile_x in 0..8 {
                    let sprite_pixel = self.sprite_data.get_sprite_pixel(tile_x, cgb, vram);
                    if self.spr_fifo.len() <= tile_x {
                        self.spr_fifo.push_back(sprite_pixel);
                    } else if self.spr_fifo[tile_x].pixel.color == 0 ||
                        (cgb && self.sprite_data.has_priority_over_sprite(self.spr_fifo[tile_x].oam_offset))
                    {
                        self.spr_fifo[tile_x] = sprite_pixel;
                    }
                }
                self.rendering_sprites = false;
            }
        } else if self.bg_fifo.len() <= 8 {
            let tile_x = 7 - self.scroll_quantity as usize;
            self.scroll_quantity = 0;
            for x in (0..=tile_x).rev() {
                let pixel = self.tile_data.get_tile_pixel(cgb, x);
                // TODO cgb
                self.bg_fifo.push_back(pixel);
            }
            self.tile_data.increment_row_index();
        }
        // self.dot_clock_divider = true;
        self.state = PixelFetcherState::GetTile;
    }

    #[inline]
    pub fn clear_queues(&mut self) {
        self.bg_fifo.clear();
        self.spr_fifo.clear();
    }

    pub fn pop_bg(&mut self) -> Option<TilePixel> {
        self.bg_fifo.pop_front()
    }

    pub fn pop_spr(&mut self) -> Option<SpritePixel> {
        self.spr_fifo.pop_front()
    }
    
    pub fn spr_front(&self) -> Option<&SpritePixel> {
        self.spr_fifo.front()
    }

    pub fn is_bg_fifo_full(&self) -> bool {
        self.bg_fifo.len() >= 8
    }

    pub fn is_spr_fifo_full(&self) -> bool {
        self.spr_fifo.len() >= 8
    }
}

#[derive(Default)]
pub struct TilePixel {
    pub color: u8,
    pub palette: u8,
    pub priority: bool
}

pub struct SpritePixel {
    pub(crate) pixel: TilePixel,
    pub(crate) oam_offset: u8
}