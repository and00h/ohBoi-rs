use std::cell::RefCell;
use std::cmp::Ordering;
use std::collections::VecDeque;
use std::rc::Rc;
use log::{debug, trace, warn};
use crate::core::interrupts::{Interrupt, InterruptController};

const WIDTH: usize = 160;
const HEIGHT: usize = 144;
const DMG_PALETTE: [[u8; 4]; 4] = [
    [0xFF, 0xFF, 0xFF, 0xFF],
    [0xCC, 0xCC, 0xCC, 0xFF],
    [0x77, 0x77, 0x77, 0xFF],
    [0x00, 0x00, 0x00, 0xFF]
];

#[repr(u8)]
#[derive(Clone, Copy)]
pub (in crate::core) enum PpuState {
    HBlank = 0,
    VBlank,
    OAMSearch,
    PixelTransfer
}

mod lcdc_flags {
    pub const BG_WINDOW_ENABLE_PRIORITY: u8 = 0b00000001;
    pub const OBJ_ENABLE: u8                = 0b00000010;
    pub const OBJ_SIZE: u8                  = 0b00000100;
    pub const BG_TILE_MAP: u8               = 0b00001000;
    pub const BG_WINDOW_TILE_DATA: u8       = 0b00010000;
    pub const WINDOW_ENABLE: u8             = 0b00100000;
    pub const WINDOW_TILE_MAP: u8           = 0b01000000;
    pub const LCD_ENABLE: u8                = 0b10000000;
}

mod lcd_stat_flags {
    pub const MODE: u8                  = 0b00000011;
    pub const LY_COMPARE: u8            = 0b00000100;
    pub const HBLANK_INTERRUPT: u8      = 0b00001000;
    pub const VBLANK_INTERRUPT: u8      = 0b00010000;
    pub const OAM_INTERRUPT: u8         = 0b00100000;
    pub const LY_COMPARE_INTERRUPT: u8  = 0b01000000;
    pub const UNUSED: u8                = 0b10000000;
}

mod sprite_attr_flags {
    pub const CGB_PALETTE_NUMBER: u8    = 0b00000111;
    pub const VRAM_BANK: u8             = 0b00001000;
    pub const DMG_PALETTE_NUMBER: u8    = 0b00010000;
    pub const X_FLIP: u8                = 0b00100000;
    pub const Y_FLIP: u8                = 0b01000000;
    pub const BG_WINDOW_PRIORITY: u8    = 0b10000000;
}

#[derive(Default, Copy, Clone)]
struct Tile {
    pub colors: [[u8; 8]; 8],
    tile_data: [u8; 16]
}

impl Tile {
    pub fn new(data: &[u8]) -> Self {
        let mut res = Self {
            colors: [[0; 8]; 8],
            tile_data: [0; 16],
        };

        for y in 0..8 {
            let tile_line = y * 2;
            let data1 = data[tile_line];
            let data2 = data[tile_line + 1];
            res.tile_data[tile_line] = data1;
            res.tile_data[tile_line + 1] = data2;
            for x in 0..8 {
                res.colors[y][x] = ((data2 >> x) & 1) << 1 | ((data1 >> x) & 1);
            }
        }

        res
    }

    pub fn update_byte(&mut self, n: usize, val: u8) {
        self.tile_data[n] = val;
        let line = n & 0xFE;
        let y = line / 2;
        for x in 0..8 {
            self.colors[y][x] = ((self.tile_data[line + 1] >> x) & 1) << 1 | ((self.tile_data[line] >> x) & 1);
        }
    }
}

#[derive(Default)]
struct TilePixel {
    pub color: u8,
    pub palette: u8,
    pub priority: bool
}

struct SpritePixel {
    pixel: TilePixel,
    oam_offset: u8
}

#[derive(Copy, Clone, Default)]
struct Sprite {
    pub oam_offset: u8,
    pub y: u8,
    pub x: u8,
    pub tile_location: u8,
    pub attributes: u8,
    pub removed: bool
}

impl Sprite {
    pub fn new(oam_offset: u8, oam_data: &[u8]) -> Self {
        Self {
            oam_offset,
            y: oam_data[0],
            x: oam_data[1],
            tile_location: oam_data[2],
            attributes: oam_data[3],
            removed: false
        }
    }

    pub fn cgb_palette_number(&self) -> u8 {
        self.attributes & sprite_attr_flags::CGB_PALETTE_NUMBER
    }
    pub fn x_flip(&self) -> bool {
        self.attributes & sprite_attr_flags::X_FLIP != 0
    }
    pub fn y_flip(&self) -> bool {
        self.attributes & sprite_attr_flags::Y_FLIP != 0
    }
    pub fn vram_bank(&self) -> u8 {
        self.attributes & sprite_attr_flags::VRAM_BANK
    }
    pub fn dmg_palette_number(&self) -> u8 {
        (self.attributes & sprite_attr_flags::DMG_PALETTE_NUMBER) >> 4
    }
    pub fn has_priority(&self) -> bool {
        self.attributes & sprite_attr_flags::BG_WINDOW_PRIORITY == 0
    }
}

enum PixelFetcherState {
    GetTile,
    GetTileDataLo,
    GetTileDataHi,
    Sleep,
    Push
}

struct PixelFetcher {
    current_sprite: Sprite,
    state: PixelFetcherState,

    tile_row_index: u16,
    tile_row_addr: u16,
    tile_index: i32,
    tile_y: u8,

    scroll_quantity: u8,

    sprite_tile_index: u8,
    sprite_tile_y: u8,
    rendering_sprites: bool,

    dot_clock_divider: bool,
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
        }
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

    pub fn step(&mut self, vram: &[u8], tileset: &[Tile], bg_fifo: &mut VecDeque<TilePixel>, spr_fifo: &mut VecDeque<SpritePixel>, signed_tileset: bool) {
        match self.state {
            PixelFetcherState::GetTile => self.get_tile(vram, signed_tileset),
            PixelFetcherState::GetTileDataLo => self.get_tile_data_lo(),
            PixelFetcherState::GetTileDataHi => self.get_tile_data_hi(bg_fifo, spr_fifo),
            PixelFetcherState::Sleep => self.sleep(),
            PixelFetcherState::Push => self.push(tileset, bg_fifo, spr_fifo)
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

    fn get_tile_data_hi(&mut self, bg_fifo: &VecDeque<TilePixel>, spr_fifo: &VecDeque<SpritePixel>) {
        self.step_dot_clock();
        if self.dot_clock_divider {
            self.state = if self.rendering_sprites {
                if spr_fifo.len() <= 8 {
                    PixelFetcherState::Push
                } else {
                    PixelFetcherState::Sleep
                }
            } else {
                if bg_fifo.len() <= 8 {
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

    fn push(&mut self, tileset: &[Tile], bg_fifo: &mut VecDeque<TilePixel>, spr_fifo: &mut VecDeque<SpritePixel>) {
        if self.rendering_sprites {
            if spr_fifo.len() <= 8 {
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
                    if spr_fifo.len() <= tile_x {
                        spr_fifo.push_back(sprite_pixel);
                    } else if spr_fifo[tile_x].pixel.color == 0 /* || TODO roba cgb */ {
                        spr_fifo[tile_x] = sprite_pixel;
                    }
                }
                self.rendering_sprites = false;
            }
        } else {
            if bg_fifo.len() <= 8 {
                let tile_x = 7 - self.scroll_quantity as usize;
                self.scroll_quantity = 0;
                for x in (0..=tile_x).rev() {
                    let pixel = TilePixel {
                        color: tileset[self.tile_index as usize].colors[self.tile_y as usize][x],
                        palette: 0,
                        priority: false,
                    };
                    // TODO cgb
                    bg_fifo.push_back(pixel);
                }
                self.tile_row_index = (self.tile_row_index + 1) & 0x1F;
            }
        }
        // self.dot_clock_divider = true;
        self.state = PixelFetcherState::GetTile;
    }
}

struct DmgPalette {
    value: u8,
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

pub struct Ppu {
    interrupt_controller: Rc<RefCell<InterruptController>>,
    screen: [u8; WIDTH * HEIGHT * 4],
    vram: [u8; 0x2000],
    oam: [u8; 0xA0],
    pub (in crate::core) state: PpuState,
    tileset0: [Tile; 384],
    tileset1: [Tile; 384],
    sprites: Vec<Sprite>,
    bg_fifo: VecDeque<TilePixel>,
    spr_fifo: VecDeque<SpritePixel>,
    lcdc: u8,
    lcd_stat: u8,
    scroll_x: u8,
    scroll_y: u8,
    ly: usize,
    ly_compare: u8,
    window_x: u8,
    window_y: u8,
    bg_palette: DmgPalette,
    obj0_palette: DmgPalette,
    obj1_palette: DmgPalette,
    vram_bank: u8,
    internal_window_counter: u8,
    current_pixel: u8,
    scanline_counter: u32,
    pixel_fetcher: PixelFetcher,
    rendering_window: bool
}

impl Ppu {
    pub fn new(interrupt_controller: Rc<RefCell<InterruptController>>) -> Self {
        use lcdc_flags::*;
        use lcd_stat_flags::*;
        Self {
            interrupt_controller,
            screen: [0; WIDTH * HEIGHT * 4],
            vram: [0; 0x2000],
            oam: [0; 0xA0],
            state: PpuState::VBlank,
            tileset0: [Tile::default(); 384],
            tileset1: [Tile::default(); 384],
            sprites: Vec::new(),
            bg_fifo: VecDeque::new(),
            spr_fifo: VecDeque::new(),
            lcdc: LCD_ENABLE | BG_WINDOW_TILE_DATA | BG_WINDOW_ENABLE_PRIORITY,
            lcd_stat: UNUSED | PpuState::VBlank as u8,

            scroll_x: 0,
            scroll_y: 0,
            ly: 0,
            ly_compare: 0,
            window_x: 0,
            window_y: 0,
            bg_palette: DmgPalette::new(0xFC),
            obj0_palette: DmgPalette::new(0xFF),
            obj1_palette: DmgPalette::new(0xFF),
            vram_bank: 0,
            internal_window_counter: 0,
            current_pixel: 0,
            scanline_counter: 0,
            pixel_fetcher: PixelFetcher::new(),
            rendering_window: true
        }
    }

    fn read_vram(&self, addr: u16) -> u8 {
        if matches!(self.state, PpuState::PixelTransfer) {
            trace!("Reading from blocked VRAM (addr {:04X})", addr);
            0xFF
        } else {
            trace!("Reading from VRAM (addr {:04X})", addr);
            self.vram[addr as usize]
        }
    }

    fn write_vram(&mut self, addr: u16, val: u8) {
        if !matches!(self.state, PpuState::PixelTransfer) {
            trace!("Writing val {:02X} to VRAM addr {:04X})", val, addr);
            self.vram[addr as usize] = val;
            if addr < 0x1800 {
                self.tileset0[(addr >> 4) as usize].update_byte((addr & 0xF) as usize, val);
            }
        }
    }

    fn read_oam(&self, addr: u16, dma: bool) -> u8 {
        match self.state {
            _ if dma => self.oam[addr as usize],
            PpuState::OAMSearch | PpuState::PixelTransfer => 0xFF,
            PpuState::VBlank | PpuState::HBlank => self.oam[addr as usize]
        }
    }

    fn write_oam(&mut self, addr: u16, val: u8, dma: bool) {
        match self.state {
            _ if dma => self.oam[addr as usize] = val,
            PpuState::OAMSearch | PpuState::PixelTransfer => {},
            PpuState::VBlank | PpuState::HBlank => self.oam[addr as usize] = val
        }
    }

    pub fn read(&self, addr: u16, dma: bool) -> u8 {
        match addr {
            0x8000..=0x9FFF => self.read_vram(addr - 0x8000),
            0xFE00..=0xFE9F => self.read_oam(addr - 0xFE00, dma),
            0xFF40 => self.lcdc,
            0xFF41 => self.lcd_stat,
            0xFF42 => self.scroll_y,
            0xFF43 => self.scroll_x,
            0xFF44 => self.ly as u8,
            0xFF45 => self.ly_compare,
            0xFF47 => self.bg_palette.value,
            0xFF48 => self.obj0_palette.value,
            0xFF49 => self.obj1_palette.value,
            0xFF4A => self.window_y,
            0xFF4B => self.window_x,
            0xFF4F => self.vram_bank | 0xFE,
            _ => {
                warn!("Read from unimplemented I/O port: {:04X}", addr);
                0xFF
            }
        }
    }

    pub fn write(&mut self, addr: u16, val: u8, dma: bool) {
        match addr {
            0x8000..=0x9FFF => self.write_vram(addr - 0x8000, val),
            0xFE00..=0xFE9F => self.write_oam(addr - 0xFE00, val, dma),
            0xFF40 => {
                self.lcdc = val;
                if self.lcdc & lcdc_flags::LCD_ENABLE == 0 {
                    self.disable_lcd();
                }
                // println!("{:02X} {:02X}", self.lcdc, lcdc_flags::LCD_ENABLE);
            },
            0xFF41 => self.lcd_stat = 0x80 | val,
            0xFF42 => self.scroll_y = val,
            0xFF43 => self.scroll_x = val,
            0xFF44 => self.ly = 0,
            0xFF45 => self.ly_compare = val,
            0xFF47 => self.bg_palette.update_palette(val),
            0xFF48 => self.obj0_palette.update_palette(val),
            0xFF49 => self.obj1_palette.update_palette(val),
            0xFF4A => self.window_y = val,
            0xFF4B => self.window_x = val,
            0xFF4F => self.vram_bank = val & 1,
            _ => warn!("Write of value {:02X} to unimplemented I/O port: {:04X}", val, addr)
        }
    }

    pub fn clock(&mut self) {
        if self.lcdc & lcdc_flags::LCD_ENABLE == 0 {
            return;
        }
        for _ in 0..4 {
            self.advance_scanline_counter();
            match self.state {
                PpuState::HBlank if self.scanline_counter == 0 => self.hblank(),
                PpuState::VBlank if self.scanline_counter == 0 => self.vblank(),
                PpuState::OAMSearch if self.scanline_counter == 80 => self.oam_search(),
                PpuState::PixelTransfer => {
                    self.pixel_transfer();
                    self.step_pixel_fetcher();
                },
                _ => {}
            };
        }

    }

    fn update_state(&mut self, new_state: PpuState) {
        self.state = new_state;

        self.lcd_stat &= 0xFC;
        self.lcd_stat |= new_state as u8;
        let interrupt_mask = match new_state {
            PpuState::HBlank => lcd_stat_flags::HBLANK_INTERRUPT,
            PpuState::VBlank => lcd_stat_flags::VBLANK_INTERRUPT,
            PpuState::OAMSearch => lcd_stat_flags::OAM_INTERRUPT,
            _ => 0
        };
        if self.lcd_stat & interrupt_mask != 0 {
            (*self.interrupt_controller).borrow_mut().raise(Interrupt::LCD);
        }
    }

    fn hblank(&mut self) {
        self.advance_scanline();
        if self.ly == 144 {
            (*self.interrupt_controller).borrow_mut().raise(Interrupt::VBLANK);
            self.update_state(PpuState::VBlank);
        } else {
            self.update_state(PpuState::OAMSearch)
        }
    }

    fn vblank(&mut self) {
        self.advance_scanline();
        if self.ly == 0 {
            self.internal_window_counter = 0;
            self.update_state(PpuState::OAMSearch);
        }
    }

    fn oam_search(&mut self) {
        self.sprites.clear();
        for i in (0..0xA0).step_by(4) {
            let oam_index = i as usize;
            let sprite = Sprite::new(i, &self.oam[oam_index..oam_index + 4]);
            let obj_size = if self.lcdc & lcdc_flags::OBJ_SIZE != 0 { 16 } else { 8 };
            let visible_range = (sprite.y..(sprite.y.wrapping_add(obj_size)));
            if visible_range.contains(&(self.ly as u8 + 16)) {
                self.sprites.push(sprite);
            }
            if self.sprites.len() == 10 {
                break;
            }
        }
        self.sprites.sort_by(|a, b| a.x.cmp(&b.x));

        let x = self.scroll_x;
        let y = (self.ly as u8).wrapping_add(self.scroll_y);
        self.rendering_window = false;
        let tilemap = if self.lcdc & lcdc_flags::BG_TILE_MAP == 0 { 0x1800 } else { 0x1C00 };
        self.pixel_fetcher.start(x, y, tilemap, self.scroll_x & 0b111);

        self.bg_fifo.clear();
        self.spr_fifo.clear();

        self.update_state(PpuState::PixelTransfer);
    }

    fn pixel_transfer(&mut self) {
        if self.pixel_fetcher.rendering_sprites {
            return;
        }

        if self.lcdc & lcdc_flags::OBJ_ENABLE != 0 {
            let sprite =
                self.sprites
                    .iter_mut()
                    .find(|s| ((s.x.wrapping_sub(8))..s.x).contains(&self.current_pixel) && !s.removed);

            if let Some(sprite) = sprite {
                if self.bg_fifo.len() >= 8 {
                    self.pixel_fetcher.start_sprite_fetch(*sprite, self.lcdc & lcdc_flags::OBJ_SIZE != 0, self.ly as u8);
                    sprite.removed = true;
                    return;
                }
            }
        }

        if !self.rendering_window && self.is_window_visible() {
            self.rendering_window = true;

            let x = self.current_pixel.wrapping_sub(self.window_x.wrapping_sub(7));
            let y = self.internal_window_counter;
            let tilemap =
                if self.lcdc & lcdc_flags::WINDOW_TILE_MAP != 0 {
                    0x1C00
                } else {
                    0x1800
                };
            self.pixel_fetcher.start(x, y, tilemap, 0);
            self.bg_fifo.clear();

            return;
        }

        if self.bg_fifo.len() >= 8 {
            let tile_pixel =
                match self.bg_fifo.pop_front() {
                    Some(pixel) if self.lcdc & lcdc_flags::BG_WINDOW_ENABLE_PRIORITY != 0 => pixel,
                    _ => TilePixel::default()
            };
            let mut color = tile_pixel.color;
            let mut palette = self.bg_palette.colors();
            if let Some(sprite_pixel) = self.spr_fifo.pop_front() {
                if sprite_pixel.pixel.color != 0
                    && (self.lcdc & lcdc_flags::BG_WINDOW_ENABLE_PRIORITY == 0
                    || sprite_pixel.pixel.priority || tile_pixel.color == 0) {
                    color = sprite_pixel.pixel.color;
                    palette = if sprite_pixel.pixel.palette == 0 {
                        self.obj0_palette.colors()
                    } else {
                        self.obj1_palette.colors()
                    };
                }
            }

            let pixel = ((self.ly as usize) * 160 + self.current_pixel as usize) * 4;
            self.screen[pixel..pixel+4].copy_from_slice(&palette[color as usize]);
            self.advance_x();
        }
    }

    fn advance_x(&mut self) {
        self.current_pixel += 1;
        if self.current_pixel == 160 {
            self.current_pixel = 0;
            if self.rendering_window {
                self.internal_window_counter += 1;
            }
            self.update_state(PpuState::HBlank);
        }
    }
    fn step_pixel_fetcher(&mut self) {
        let signed_tileset = self.lcdc & lcdc_flags::BG_WINDOW_TILE_DATA == 0;
        self.pixel_fetcher.step(&self.vram,
                                &self.tileset0, // tileset1 used only by cgb
                                &mut self.bg_fifo,
                                &mut self.spr_fifo,
                                signed_tileset);

    }

    fn advance_scanline_counter(&mut self) {
        self.scanline_counter = (self.scanline_counter + 1) % 456;
    }
    fn advance_scanline(&mut self) {
        self.ly += 1;
        if self.ly == 154 {
            self.ly = 0;
        }
        if (self.ly as u8) == self.ly_compare {
            self.lcd_stat |= lcd_stat_flags::LY_COMPARE;
            (*self.interrupt_controller).borrow_mut().raise(Interrupt::LCD);
        } else if self.lcd_stat & lcd_stat_flags::LY_COMPARE != 0 {
            self.lcd_stat &= !lcd_stat_flags::LY_COMPARE;
        }
    }
    fn is_window_visible(&self) -> bool {
        let window_enabled = (self.lcdc & lcdc_flags::WINDOW_ENABLE) != 0;
        let y_visible = self.window_y <= self.ly as u8 && (0..=143).contains(&self.window_y);
        let x_visible = self.current_pixel >= (self.window_x.wrapping_sub(7)) && (0..=166).contains(&self.window_x);
        window_enabled && y_visible && x_visible
    }
    fn disable_lcd(&mut self) {
        self.scanline_counter = 0;
        self.ly = 0;
        self.lcd_stat &= 0xFC;
        self.state = PpuState::VBlank;
    }

    pub fn screen(&self) -> &[u8] {
        &self.screen
    }
}