mod fifo;
mod palettes;

use std::cell::RefCell;
use std::rc::Rc;
use bitfield::bitfield;
use log::{trace, warn};
use fifo::PixelFetcher;
use palettes::DmgPalette;
use crate::core::interrupts::{Interrupt, InterruptController};
use crate::core::ppu::palettes::CgbPalette;

const WIDTH: usize = 160;
const HEIGHT: usize = 144;

#[repr(u8)]
#[derive(Clone, Copy)]
pub (in crate::core) enum PpuState {
    HBlank = 0,
    VBlank,
    OAMSearch,
    PixelTransfer
}

bitfield! {
    struct Lcdc(u8);
    impl Debug;
    pub bg_window_enable_priority, set_bg_window_enable_priority: 0;
    pub obj_enable, set_obj_enable: 1;
    pub obj_size, set_obj_size: 2;
    pub bg_tile_map, set_bg_tile_map: 3;
    pub bg_window_tile_data, set_bg_window_tile_data: 4;
    pub window_enable, set_window_enable: 5;
    pub window_tile_map, set_window_tile_map: 6;
    pub lcd_enable, set_lcd_enable: 7;
}

bitfield! {
    struct LcdStat(u8);
    impl Debug;
    pub state, set_state: 1, 0;
    pub ly_compare, set_ly_compare: 2;
    pub hblank_interrupt, set_hblank_interrupt: 3;
    pub vblank_interrupt, set_vblank_interrupt: 4;
    pub oam_interrupt, set_oam_interrupt: 5;
    pub ly_compare_interrupt, set_ly_compare_interrupt: 6;
    pub unused, set_unused: 7;
}

mod lcd_stat_interrupts {
    pub const HBLANK_INTERRUPT: u8      = 0b00001000;
    pub const VBLANK_INTERRUPT: u8      = 0b00010000;
    pub const OAM_INTERRUPT: u8         = 0b00100000;
    pub const LY_COMPARE_INTERRUPT: u8  = 0b01000000;
}

bitfield! {
    #[derive(Copy, Clone, Default)]
    pub struct SpriteAttributes(u8);
    impl Debug;
    pub cgb_palette_number, set_cgb_palette_number: 2, 0;
    pub vram_bank, set_vram_bank: 3;
    pub dmg_palette_number, set_dmg_palette_number: 4;
    pub x_flip, set_x_flip: 5;
    pub y_flip, set_y_flip: 6;
    pub bg_window_priority, set_bg_window_priority: 7;
}

mod dmg_palettes {
    pub(crate) const BG: usize = 0;
    pub(crate) const OBJ0: usize = 1;
    pub(crate) const OBJ1: usize = 2;
}

bitfield! {
    #[derive(Copy, Clone, Default)]
    pub struct TileAttributes(u8);
    impl Debug;
    pub palette, _: 2, 0;
    pub bank, _: 3;
    pub x_flip, _: 5;
    pub y_flip, _: 6;
    pub priority, _: 7;
}

enum Tileset<'a> {
    Dmg(&'a[Tile]),
    Cgb(&'a[Tile], &'a[Tile])
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
    pub fn vram_bank(&self) -> u8 {
        self.attributes.vram_bank() as u8
    }
    pub fn dmg_palette_number(&self) -> u8 {
        self.attributes.dmg_palette_number() as u8
    }
    pub fn has_priority(&self) -> bool {
        !self.attributes.bg_window_priority()
    }
}

struct Window {
    x: u8,
    y: u8,
    rendering: bool,
    internal_line_counter: u8
}

pub struct Ppu {
    interrupt_controller: Rc<RefCell<InterruptController>>,
    screen: [u8; WIDTH * HEIGHT * 4],
    vram: Vec<u8>,
    oam: [u8; 0xA0],
    pub (in crate::core) state: PpuState,
    tileset0: [Tile; 384],
    tileset1: Option<[Tile; 384]>,
    sprites: Vec<Sprite>,
    lcdc: Lcdc,
    lcd_stat: LcdStat,
    scroll_x: u8,
    scroll_y: u8,
    ly: usize,
    ly_compare: u8,
    window: Window,
    dmg_palettes: [DmgPalette; 3],
    current_pixel: u8,
    scanline_counter: u32,
    pixel_fetcher: PixelFetcher,

    vram_bank: u8,
    cgb_bg_pal: Option<CgbPalette>,
    cgb_obj_pal: Option<CgbPalette>,
    cgb: bool
}

impl Ppu {
    pub fn new(interrupt_controller: Rc<RefCell<InterruptController>>, cgb: bool) -> Self {
        let vram = if cgb { vec![0; 0x4000] } else { vec![0; 0x2000] };
        let tileset1 = if cgb { Some([Tile::default(); 384]) } else { None };
        let cgb_bg_pal = if cgb { Some(CgbPalette::new()) } else { None };
        let cgb_obj_pal = if cgb { Some(CgbPalette::new()) } else { None };
        
        Self {
            interrupt_controller,
            screen: [0; WIDTH * HEIGHT * 4],
            vram,
            oam: [0; 0xA0],
            state: PpuState::VBlank,
            tileset0: [Tile::default(); 384],
            tileset1,
            sprites: Vec::new(),
            lcdc: Lcdc(0x91),
            lcd_stat: LcdStat(0x80 | PpuState::VBlank as u8),
            scroll_x: 0,
            scroll_y: 0,
            ly: 0,
            ly_compare: 0,
            window: Window { x: 0, y: 0, rendering: true, internal_line_counter: 0 },
            dmg_palettes: [DmgPalette::new(0xFC), DmgPalette::new(0xFF), DmgPalette::new(0xFF)],
            vram_bank: 0,
            current_pixel: 0,
            scanline_counter: 0,
            pixel_fetcher: PixelFetcher::new(),
            cgb_bg_pal,
            cgb_obj_pal,
            cgb
        }
    }

    pub fn reset(&mut self) {
        self.state = PpuState::VBlank;

        self.lcdc.set_lcd_enable(true);
        self.lcdc.set_bg_window_tile_data(true);
        self.lcdc.set_bg_window_enable_priority(true);

        self.lcd_stat.set_unused(true);
        self.lcd_stat.set_state(PpuState::VBlank as u8);

        self.scroll_x = 0;
        self.scroll_y = 0;
        self.ly = 0;
        self.ly_compare = 0;
        self.window.x = 0;
        self.window.y = 0;
        self.window.rendering = true;
        self.window.internal_line_counter = 0;

        self.dmg_palettes = [
            DmgPalette::new(0xFC),
            DmgPalette::new(0xFF),
            DmgPalette::new(0xFF)
        ];
        self.vram_bank = 0;
        self.current_pixel = 0;
        self.scanline_counter = 0;
        self.pixel_fetcher.reset();
        self.sprites.clear();

        self.screen = [0; WIDTH * HEIGHT * 4];
        self.vram.fill(0);
        self.oam = [0; 0xA0];
        self.tileset0 = [Tile::default(); 384];
        self.tileset1 = if self.cgb { Some([Tile::default(); 384]) } else { None };

    }

    fn read_vram(&self, addr: u16, bank: usize) -> u8 {
        if matches!(self.state, PpuState::PixelTransfer) {
            trace!("Reading from blocked VRAM (addr {:04X})", addr);
            0xFF
        } else {
            trace!("Reading from VRAM (addr {:04X})", addr);
            self.vram[addr as usize + (bank * 0x2000)]
        }
    }

    fn write_vram(&mut self, addr: u16, val: u8, bank: usize) {
        if !matches!(self.state, PpuState::PixelTransfer) {
            trace!("Writing val {:02X} to VRAM addr {:04X})", val, addr);
            self.vram[addr as usize + bank * 0x2000] = val;
            if addr < 0x1800 {
                if self.vram_bank == 0 {
                    self.tileset0[(addr >> 4) as usize].update_byte((addr & 0xF) as usize, val);
                } else {
                    let mut t1 = self.tileset1.unwrap();
                    t1[(addr >> 4) as usize].update_byte((addr & 0xF) as usize, val);
                    self.tileset1.replace(t1);
                }
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
            0x8000..=0x9FFF => self.read_vram(addr - 0x8000, self.vram_bank as usize),
            0xFE00..=0xFE9F => self.read_oam(addr - 0xFE00, dma),
            0xFF40 => self.lcdc.0,
            0xFF41 => self.lcd_stat.0 & (if !self.lcdc.lcd_enable() { 0xFC } else { 0xFF }),
            0xFF42 => self.scroll_y,
            0xFF43 => self.scroll_x,
            0xFF44 => self.ly as u8,
            0xFF45 => self.ly_compare,
            0xFF47 => self.dmg_palettes[dmg_palettes::BG].value,
            0xFF48 => self.dmg_palettes[dmg_palettes::OBJ0].value,
            0xFF49 => self.dmg_palettes[dmg_palettes::OBJ1].value,
            0xFF4A => self.window.y,
            0xFF4B => self.window.x,
            0xFF4F => self.vram_bank | 0xFE,
            0xFF68 => self.cgb_bg_pal.as_ref().map_or(0xFF, |pal| pal.index.0),
            0xFF69 => self.cgb_bg_pal.as_ref().map_or(0xFF, |pal| pal.read_data()),
            0xFF6A => self.cgb_obj_pal.as_ref().map_or(0xFF, |pal| pal.index.0),
            0xFF6B => self.cgb_obj_pal.as_ref().map_or(0xFF, |pal| pal.read_data()),
            _ => {
                warn!("Read from unimplemented I/O port: {:04X}", addr);
                0xFF
            }
        }
    }

    pub fn write(&mut self, addr: u16, val: u8, dma: bool) {
        match addr {
            0x8000..=0x9FFF => self.write_vram(addr - 0x8000, val, self.vram_bank as usize),
            0xFE00..=0xFE9F => self.write_oam(addr - 0xFE00, val, dma),
            0xFF40 => {
                self.lcdc.0 = val;
                if !self.lcdc.lcd_enable() {
                    self.disable_lcd();
                }
            },
            0xFF41 => self.lcd_stat.0 = 0x80 | val,
            0xFF42 => self.scroll_y = val,
            0xFF43 => self.scroll_x = val,
            0xFF44 => self.ly = 0,
            0xFF45 => self.ly_compare = val,
            0xFF47..=0xFF49 => self.dmg_palettes[(addr - 0xFF47) as usize].update_palette(val),
            0xFF4A => self.window.y = val,
            0xFF4B => self.window.x = val,
            0xFF4F if self.cgb => self.vram_bank = val & 1,
            0xFF68 => if let Some(ref mut pal) = self.cgb_bg_pal { pal.write_index(val) },
            0xFF69 => if let Some(ref mut pal) = self.cgb_bg_pal { 
                if matches!(self.state, PpuState::PixelTransfer) { 
                    pal.increment_index(); 
                } else {
                    pal.write_data(val)
                }
            },
            0xFF6A => if let Some(ref mut pal) = self.cgb_obj_pal { pal.write_index(val) },
            0xFF6B => if let Some(ref mut pal) = self.cgb_obj_pal { 
                if matches!(self.state, PpuState::PixelTransfer) { 
                    pal.increment_index(); 
                } else {
                    pal.write_data(val)
                }
            },
            _ => warn!("Write of value {:02X} to unimplemented I/O port: {:04X}", val, addr)
        }
    }

    pub fn clock(&mut self) {
        if !self.lcdc.lcd_enable() {
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

        self.lcd_stat.set_state(new_state as u8);
        let interrupt_mask = match new_state {
            PpuState::HBlank => lcd_stat_interrupts::HBLANK_INTERRUPT,
            PpuState::VBlank => lcd_stat_interrupts::VBLANK_INTERRUPT,
            PpuState::OAMSearch => lcd_stat_interrupts::OAM_INTERRUPT,
            _ => 0
        };
        if self.lcd_stat.0 & interrupt_mask != 0 {
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
            self.window.internal_line_counter = 0;
            self.update_state(PpuState::OAMSearch);
        }
    }

    fn oam_search(&mut self) {
        self.sprites.clear();
        for i in (0..0xA0).step_by(4) {
            let oam_index = i as usize;
            let sprite = Sprite::new(i, &self.oam[oam_index..oam_index + 4]);
            let obj_size = if self.lcdc.obj_size() { 16 } else { 8 };
            let visible_range = sprite.y..sprite.y.wrapping_add(obj_size);
            if visible_range.contains(&(self.ly as u8 + 16)) {
                self.sprites.push(sprite);
            }
            if self.sprites.len() == 10 {
                break;
            }
        }
        if !self.cgb {
            self.sprites.sort_by(|a, b| a.x.cmp(&b.x));
        }

        let x = self.scroll_x;
        let y = (self.ly as u8).wrapping_add(self.scroll_y);
        self.window.rendering = false;
        let tilemap = if self.lcdc.bg_tile_map() { 0x1C00 } else { 0x1800 };
        self.pixel_fetcher.clear_queues();
        self.pixel_fetcher.start(x, y, tilemap, self.scroll_x & 0b111);

        self.update_state(PpuState::PixelTransfer);
    }

    fn pixel_transfer(&mut self) {
        if self.pixel_fetcher.rendering_sprites {
            return;
        }

        if self.lcdc.obj_enable() {
            // Check if we have a sprite to render
            // Sprites may be rendered if the following conditions are met:
            // 1. The sprite is enabled
            // 2. The sprite overlaps with the current pixel
            let sprite =
                self.sprites
                    .iter_mut()
                    .enumerate()
                    .find(|(_, s)| s.x != 0 && (s.x.saturating_sub(8)..s.x).contains(&self.current_pixel));

            if let Some((index, sprite)) = sprite {
                if self.pixel_fetcher.is_bg_fifo_full() {
                    self.pixel_fetcher.start_sprite_fetch(*sprite, self.lcdc.obj_size(), self.ly as u8);
                    self.sprites.remove(index);
                    return;
                }
            }
        }

        // Start window rendering if the PPU is not currently rendering it, but it became visible
        if !self.window.rendering && self.is_window_visible() {
            self.window.rendering = true;

            let x = self.current_pixel.wrapping_sub(self.window.x.wrapping_sub(7));
            let y = self.window.internal_line_counter;
            let tilemap =
                if self.lcdc.window_tile_map() {
                    0x1C00
                } else {
                    0x1800
                };
            self.pixel_fetcher.start(x, y, tilemap, 0);

            return;
        }


        if self.pixel_fetcher.is_bg_fifo_full() {
            let tile_pixel =
                match self.pixel_fetcher.pop_bg() {
                    Some(pixel) if self.lcdc.bg_window_enable_priority() || self.cgb => pixel,
                    _ => TilePixel::default()
            };
            let mut color = tile_pixel.color;
            let mut palette = if self.cgb {
                self.cgb_bg_pal.as_ref().unwrap().color_array(tile_pixel.palette as usize)
            } else {
                self.dmg_palettes[dmg_palettes::BG].colors().to_owned()
            };
            if let Some(sprite_pixel) = self.pixel_fetcher.pop_spr() {
                if sprite_pixel.pixel.color != 0 {
                    if self.cgb {
                        if !self.lcdc.bg_window_enable_priority() || (!tile_pixel.priority && sprite_pixel.pixel.priority) || tile_pixel.color == 0 {
                            color = sprite_pixel.pixel.color;
                            palette = self.cgb_obj_pal.as_ref().unwrap().color_array(sprite_pixel.pixel.palette as usize);
                        }
                    } else {
                        if !self.lcdc.bg_window_enable_priority() || sprite_pixel.pixel.priority || tile_pixel.color == 0 {
                            color = sprite_pixel.pixel.color;
                            palette = self.dmg_palettes[sprite_pixel.pixel.palette as usize + 1].colors().to_owned();
                        }
                    }
                }
            }

            let pixel = (self.ly * 160 + self.current_pixel as usize) * 4;
            self.screen[pixel..pixel+4].copy_from_slice(&palette[color as usize]);
            self.advance_x();
        }
    }

    fn advance_x(&mut self) {
        self.current_pixel = (self.current_pixel + 1) % 160;
        if self.current_pixel == 0 {
            if self.window.rendering {
                self.window.internal_line_counter += 1;
            }
            self.update_state(PpuState::HBlank);
        }
    }
    fn step_pixel_fetcher(&mut self) {
        let signed_tileset = !self.lcdc.bg_window_tile_data();
        self.pixel_fetcher.step(&self.vram,
                                if self.cgb { Tileset::Cgb(&self.tileset0, self.tileset1.as_ref().unwrap()) } else { Tileset::Dmg(&self.tileset0) },
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
            self.lcd_stat.set_ly_compare(true);
            (*self.interrupt_controller).borrow_mut().raise(Interrupt::LCD);
        } else if self.lcd_stat.ly_compare() {
            self.lcd_stat.set_ly_compare(false);
        }
    }
    fn is_window_visible(&self) -> bool {
        let window_enabled = self.lcdc.window_enable();
        let y_visible = self.ly <= 143 && self.window.y <= self.ly as u8;
        let x_visible = self.window.x <= 166 && self.current_pixel >= (self.window.x.wrapping_sub(7));
        window_enabled && y_visible && x_visible
    }
    fn disable_lcd(&mut self) {
        self.scanline_counter = 0;
        self.ly = 0;
        self.lcd_stat.set_state(PpuState::VBlank as u8);
        self.state = PpuState::VBlank;
    }

    pub fn screen(&self) -> &[u8] {
        &self.screen
    }
    
    #[cfg(feature = "debug_ui")]
    pub fn get_tileset0(&self) -> Vec<u8> {
        let mut res = Vec::new();
        for y in 0..(12 * 8) {
            for x in 0..(32 * 8) {
                let tile_y = y / 8;
                let tile_x = x / 8;
                let tile_offset = tile_y * 12 + tile_x;
                let tile = &self.tileset0[tile_offset];
                let line = y % 8;
                let pixel = x % 8;
                let color = tile.colors[line as usize][7 - pixel as usize];
                let palette = if self.cgb {
                    self.cgb_bg_pal.as_ref().unwrap().color_array(color as usize)
                } else {
                    self.dmg_palettes[dmg_palettes::BG].colors().to_owned()
                };
                res.extend(palette[color as usize].iter());
            }
        }
        
        res
    }
    
    #[cfg(feature = "debug_ui")]
    pub fn get_tileset1(&self) -> Option<Vec<u8>> {
        if let Some(ref tileset1) = self.tileset1 {
            let mut res = Vec::new();
            for y in 0..(12 * 8) {
                for x in 0..(32 * 8) {
                    let tile_y = y / 8;
                    let tile_x = x / 8;
                    let tile_offset = tile_y * 12 + tile_x;
                    let tile = &tileset1[tile_offset];
                    let line = y % 8;
                    let pixel = x % 8;
                    let color = tile.colors[line as usize][7 - pixel as usize];
                    let palette = if self.cgb {
                        self.cgb_bg_pal.as_ref().unwrap().color_array(color as usize)
                    } else {
                        self.dmg_palettes[dmg_palettes::BG].colors().to_owned()
                    };
                    res.extend(palette[color as usize].iter());
                }
            }
            Some(res)
        } else {
            None
        }
    }
}