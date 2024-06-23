use std::collections::VecDeque;
use crate::core::ppu::{Sprite, SpritePixel, Tile, TilePixel};

enum PixelFetcherState {
    GetTile,
    GetTileDataLo,
    GetTileDataHi,
    Sleep,
    Push
}

pub struct PixelFetcher {
    current_sprite: Sprite,
    state: PixelFetcherState,

    tile_row_index: u16,
    tile_row_addr: u16,
    tile_index: i32,
    tile_y: u8,

    scroll_quantity: u8,

    sprite_tile_index: u8,
    sprite_tile_y: u8,
    pub(crate) rendering_sprites: bool,

    dot_clock_divider: bool,

    bg_fifo: VecDeque<TilePixel>,
    spr_fifo: VecDeque<SpritePixel>
}

impl PixelFetcher {
    pub fn new() -> Self {
        Self {
            current_sprite: Sprite::default(),
            state: PixelFetcherState::GetTile,
            tile_row_index: 0,
            tile_row_addr: 0,
            tile_index: 0,
            tile_y: 0,
            scroll_quantity: 0,
            sprite_tile_index: 0,
            sprite_tile_y: 0,
            rendering_sprites: false,
            dot_clock_divider: false,
            bg_fifo: VecDeque::new(),
            spr_fifo: VecDeque::new(),
        }
    }

    pub fn reset(&mut self) {
        self.current_sprite = Sprite::default();
        self.state = PixelFetcherState::GetTile;
        self.tile_row_index = 0;
        self.tile_row_addr = 0;
        self.tile_index = 0;
        self.tile_y = 0;
        self.scroll_quantity = 0;
        self.sprite_tile_index = 0;
        self.sprite_tile_y = 0;
        self.rendering_sprites = false;
        self.dot_clock_divider = false;

        self.bg_fifo.clear();
        self.spr_fifo.clear();
    }

    pub fn start(&mut self, x: u8, y: u8, tile_map: u16, scroll: u8) {
        self.state = PixelFetcherState::GetTile;
        self.tile_index = 0;
        self.dot_clock_divider = false;

        self.tile_y = y & 7;
        self.tile_row_index = (x >> 3) as u16;
        self.scroll_quantity = scroll;
        self.tile_row_addr = tile_map + (((y as u16) >> 3) << 5);
        self.rendering_sprites = false;

        self.bg_fifo.clear();
    }

    pub fn start_sprite_fetch(&mut self, sprite: Sprite, use_8x16: bool, y: u8) {
        self.state = PixelFetcherState::GetTile;
        self.sprite_tile_index = sprite.tile_location;
        if use_8x16 {
            if y >= sprite.y - 8 {
                self.sprite_tile_index |= 1;
            } else {
                self.sprite_tile_index &= 0xFE;
            }
        }
        self.sprite_tile_y = (y.wrapping_sub(sprite.y)) & 0b00000111;
        self.dot_clock_divider = false;
        if sprite.y_flip() {
            self.sprite_tile_y = 7 - self.sprite_tile_y;
            if use_8x16 {
                if y >= sprite.y - 8 {
                    self.sprite_tile_index &= 0xFE;
                } else {
                    self.sprite_tile_index |= 1
                }
            }
        }
        self.rendering_sprites = true;
        self.current_sprite = sprite;
    }

    pub fn step(&mut self, vram: &[u8], tileset: &[Tile], signed_tileset: bool) {
        match self.state {
            PixelFetcherState::GetTile => self.get_tile(vram, signed_tileset),
            PixelFetcherState::GetTileDataLo => self.get_tile_data_lo(),
            PixelFetcherState::GetTileDataHi => self.get_tile_data_hi(),
            PixelFetcherState::Sleep => self.sleep(),
            PixelFetcherState::Push => self.push(tileset)
        }
    }

    fn step_dot_clock(&mut self) {
        self.dot_clock_divider = !self.dot_clock_divider;
    }

    fn get_tile(&mut self, vram: &[u8], signed_tileset: bool) {
        self.step_dot_clock();
        if self.dot_clock_divider {
            if !self.rendering_sprites {
                self.tile_index = vram[(self.tile_row_addr + self.tile_row_index) as usize] as i32;
                if signed_tileset {
                    self.tile_index = (self.tile_index as i8) as i32 + 256;
                }
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
            } else {
                if self.bg_fifo.len() <= 8 {
                    PixelFetcherState::Push
                } else {
                    PixelFetcherState::Sleep
                }
            }
        }
    }

    fn sleep(&mut self) {
        self.step_dot_clock();
        if self.dot_clock_divider {
            self.state = PixelFetcherState::Push;
        }
    }

    fn push(&mut self, tileset: &[Tile]) {
        if self.rendering_sprites {
            if self.spr_fifo.len() <= 8 {
                for tile_x in 0..8 {
                    let color = tileset[self.sprite_tile_index as usize].colors
                        [self.sprite_tile_y as usize][if self.current_sprite.x_flip() { tile_x } else { 7 - tile_x }];
                    let sprite_pixel = SpritePixel {
                        pixel: TilePixel {
                            color,
                            palette: self.current_sprite.dmg_palette_number(),
                            priority: self.current_sprite.has_priority(),
                        },
                        oam_offset: self.current_sprite.oam_offset,
                    };
                    if self.spr_fifo.len() <= tile_x {
                        self.spr_fifo.push_back(sprite_pixel);
                    } else if self.spr_fifo[tile_x].pixel.color == 0 /* || TODO roba cgb */ {
                        self.spr_fifo[tile_x] = sprite_pixel;
                    }
                }
                self.rendering_sprites = false;
            }
        } else {
            if self.bg_fifo.len() <= 8 {
                let tile_x = 7 - self.scroll_quantity as usize;
                self.scroll_quantity = 0;
                for x in (0..=tile_x).rev() {
                    let pixel = TilePixel {
                        color: tileset[self.tile_index as usize].colors[self.tile_y as usize][x],
                        palette: 0,
                        priority: false,
                    };
                    // TODO cgb
                    self.bg_fifo.push_back(pixel);
                }
                self.tile_row_index = (self.tile_row_index + 1) & 0x1F;
            }
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

    pub fn is_bg_fifo_full(&self) -> bool {
        self.bg_fifo.len() >= 8
    }

    pub fn is_spr_fifo_full(&self) -> bool {
        self.spr_fifo.len() >= 8
    }
}
