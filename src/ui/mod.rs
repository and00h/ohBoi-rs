mod widgets;

use std::collections::VecDeque;
use std::error::Error;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use cfg_if::cfg_if;

use imgui_glow_renderer::glow::{NativeTexture, PixelUnpackData};
use imgui::{Condition, StyleVar, TextureId, Textures, Ui};
use imgui_glow_renderer::Renderer;
use imgui_glow_renderer::glow::{Context, HasContext, Texture};
use imgui_sdl2_support::SdlPlatform;
use sdl2::{Sdl};
use sdl2::video::{GLContext, GLProfile, Window};
use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use crate::core::GameBoy;
use crate::core::joypad::Key;
use crate::logging::ImguiLogString;
use crate::ui::GameWindowEvent::{Close, Nothing, Open, ToggleWaveform};
use crate::ui::widgets::DisassemblyView;

#[cfg(feature = "debug_ui")]
use crate::ui::widgets::HexView;

const GB_SCREEN_WIDTH: usize = 160;
const GB_SCREEN_HEIGHT: usize = 144;

#[derive(Debug, Clone)]
pub enum GameWindowEvent {
    Close,
    Open(PathBuf),
    //KeyPress(Keycode),
    Nothing,
    ToggleWaveform
}

fn new_texture(w: usize, h: usize, gl: &Context, textures: &mut Textures<NativeTexture>) -> Result<TextureId, Box<dyn Error>> {
    let gl_texture = unsafe { gl.create_texture() }?;
    unsafe {
        gl.bind_texture(glow::TEXTURE_2D, Some(gl_texture));
        gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MIN_FILTER, glow::LINEAR as _);
        gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MAG_FILTER, glow::NEAREST as _);
        gl.tex_image_2d(
            glow::TEXTURE_2D,
            0,
            glow::RGBA as _, // When generating a texture like this, you're probably working in linear color space
            w as _,
            h as _,
            0,
            glow::RGBA,
            glow::UNSIGNED_BYTE,
            None,
        )
    }

    Ok(textures.insert(gl_texture))
}

fn sdl_event_handler(e: &Event, gb: &mut GameBoy) -> Result<bool, Box<dyn Error>> {
    match e {
        Event::KeyDown { keycode: Some(k), .. } => {
            match *k {
                Keycode::Z => gb.press(Key::Start),
                Keycode::X => gb.press(Key::Select),
                Keycode::A => gb.press(Key::A),
                Keycode::S => gb.press(Key::B),
                Keycode::Up => gb.press(Key::Up),
                Keycode::Down => gb.press(Key::Down),
                Keycode::Left => gb.press(Key::Left),
                Keycode::Right => gb.press(Key::Right),
                _ => {}
            }
        },
        Event::KeyUp { keycode: Some(k), .. } => {
            match *k {
                Keycode::Z => gb.release(Key::Start),
                Keycode::X => gb.release(Key::Select),
                Keycode::A => gb.release(Key::A),
                Keycode::S => gb.release(Key::B),
                Keycode::Up => gb.release(Key::Up),
                Keycode::Down => gb.release(Key::Down),
                Keycode::Left => gb.release(Key::Left),
                Keycode::Right => gb.release(Key::Right),
                _ => {}
            }
        },
        Event::DropFile { filename, .. } => gb.load_new_game(PathBuf::from(filename))?,
        Event::Quit { .. } => return Ok(true),
        _ => {}
    }

    Ok(false)
}

pub struct OhBoiUi {
    sdl: Sdl,
    gl: Context,
    gl_context: GLContext,
    imgui: imgui::Context,
    platform: SdlPlatform,
    sdl_window: Window,
    renderer: Renderer,
    game_window: GameWindow,
    #[cfg(feature = "debug_ui")]
    tile_window: TileWindow,
    #[cfg(feature = "debug_ui")]
    waveform_window: WaveformWindow,
    #[cfg(feature = "debug_ui")]
    rom_window: HexView,
    #[cfg(feature = "debug_ui")]
    ext_ram_window: HexView,
    #[cfg(feature = "debug_ui")]
    disasm_window: DisassemblyView,
    textures: Textures<Texture>,
    audio_device: sdl2::audio::AudioQueue<f32>,
    #[cfg(feature = "debug_ui")]
    log_buffer: Arc<Mutex<VecDeque<ImguiLogString>>>
}

impl OhBoiUi {
    pub fn new(log_buffer: Option<Arc<Mutex<VecDeque<ImguiLogString>>>>)
        -> Result<Self, Box<dyn Error>> {
        let mut imgui = imgui::Context::create();
        imgui.set_ini_filename(None);
        imgui.set_log_filename(None);
        imgui.style_mut().display_window_padding = [0.0, 0.0];
        imgui.fonts().add_font(&[imgui::FontSource::DefaultFontData { config: None }]);

        let sdl = sdl2::init()?;
        let video_subsystem = sdl.video()?;
        let gl_attr = video_subsystem.gl_attr();
        gl_attr.set_context_version(3, 3);
        gl_attr.set_context_profile(GLProfile::Core);

        let platform = SdlPlatform::new(&mut imgui);

        let video_subsystem = sdl.video()?;
        let sdl_window =
            video_subsystem.window("ohboi", 640, 480)
                .allow_highdpi()
                .opengl()
                .resizable()
                .position_centered()
                .build()?;
        let gl_context = sdl_window.gl_create_context()?;
        sdl_window.gl_make_current(&gl_context)?;
        sdl_window.subsystem().gl_set_swap_interval(1)?;
        let gl = unsafe {
            Context::from_loader_function(|s| video_subsystem.gl_get_proc_address(s) as _)
        };

        let mut textures = Textures::<Texture>::new();
        let gb_screen_texture = new_texture(GB_SCREEN_WIDTH, GB_SCREEN_HEIGHT, &gl, &mut textures)?;
        
        #[cfg(feature = "debug_ui")]
        let tile_texture = new_texture(32 * 8, 24 * 8, &gl, &mut textures)?;
        
        let renderer = Renderer::new(&gl, &mut imgui, &mut textures, false)?;
        let game_window = GameWindow::new(gb_screen_texture);
        let tile_window = TileWindow::new(tile_texture);

        let audio_context = sdl.audio().unwrap();
        let spec = sdl2::audio::AudioSpecDesired {
            freq: Some(44100),
            channels: Some(2),
            samples: Some(2048)
        };
        let audio_device = audio_context.open_queue::<f32, _>(None, &spec).unwrap();
        audio_device.resume();
        cfg_if!{
            if #[cfg(feature = "debug_ui")] {
                //let tile_window = TileWindow::new(tile_texture);
                let waveform_window = WaveformWindow::new();
                let rom_window = HexView::new("ROM".to_string());
                let ext_ram_window = HexView::new("External RAM".to_string());
                let disasm_window = DisassemblyView::new("Disassembly".to_string());
                let log_buffer = log_buffer.unwrap_or(Arc::new(Mutex::new(VecDeque::new())));
                Ok(Self { sdl, gl, gl_context, imgui, platform, sdl_window, renderer, game_window, tile_window, waveform_window, rom_window, ext_ram_window, disasm_window, textures, audio_device, log_buffer })
            } else {
                Ok(Self { sdl, gl, gl_context, imgui, platform, sdl_window, renderer, game_window, textures, audio_device })
            }
        }
    }

    #[inline]
    fn process_sdl_events(&mut self, gb: &mut GameBoy) -> Result<bool, Box<dyn Error>> {
        let mut quit = false;
        self.sdl.event_pump()?.poll_iter().for_each(|event| {
            self.platform.handle_event(&mut self.imgui, &event);
            quit = quit || sdl_event_handler(&event, gb).expect("SDL Event Handler error");
        });

        Ok(quit)
    }

    #[inline]
    fn main_menu_bar(ui: &mut Ui) -> GameWindowEvent {
        if let Some(menubar) = ui.begin_main_menu_bar() {
            if let Some(menu) = ui.begin_menu("File") {
                if ui.menu_item_config("Open").shortcut("Ctrl+O").build() {
                    if let Some(path) =
                        tinyfiledialogs::open_file_dialog("Open ROM",
                                                          "./",
                                                          Some((&["*.gb", "*.gbc"], "Gameboy ROMs")))
                    {
                        return Open(PathBuf::from(path));
                    }
                }
                if ui.menu_item_config("Close").shortcut("Alt+F4").build() {
                    return Close;
                }
                menu.end();
            }
            if let Some(menu) = ui.begin_menu("Windows") {
                if ui.menu_item_config("Waveform").selected(false).build() {
                    return ToggleWaveform;
                }
                menu.end();
            }
            menubar.end();
        }

        Nothing
    }

    pub fn audio_callback(&mut self, audio: &[f32]) {
        self.audio_device.queue_audio(audio);
    }
    pub fn show(&mut self, gb: &mut GameBoy, text: Option<String>, sample: (&[f32], &[f32], &[f32], &[f32])) -> Result<GameWindowEvent, Box<dyn Error>> {
        let quit = self.process_sdl_events(gb)?;
        if quit {
            return Ok(Close);
        }
        let event_pump = self.sdl.event_pump()?;

        self.platform.prepare_frame(&mut self.imgui, &self.sdl_window, &event_pump);
        let ui = self.imgui.new_frame();

        let menu_event = Self::main_menu_bar(ui);
        let window_size = self.sdl_window.size();
        self.game_window.show(ui, window_size, text);

        cfg_if!{ if #[cfg(feature = "debug_ui")] {
            let hex_view_width = widgets::calc_hex_view_width(ui, 16);
            let rom_pos = [330.0, 20.0];
            self.tile_window.show(ui);
            self.rom_window.show(ui, &gb.rom(), rom_pos, Some(0x4000));
            let ext_ram_pos = [330.0, 20.0 + 300.0 + 20.0];
            match gb.ext_ram() {
                Some(ram) => self.ext_ram_window.show(ui, ram, ext_ram_pos, Some(0x2000)),
                None => {}
            }
            let waveform_pos = [ui.item_rect_size()[0] + ui.cursor_pos()[0], 20.0];
            self.waveform_window.show(gb, ui, waveform_pos, sample);
            match menu_event.clone() {
                ToggleWaveform => self.waveform_window.toggle(),
                _ => {}
            }
            self.disasm_window.show(ui, gb, [ui.item_rect_size()[0] + ui.cursor_pos()[0], 20.0 + 300.0 + 20.0]);
            widgets::log_window(ui, "Log", Arc::clone(&self.log_buffer));
        }}

        let draw_data = self.imgui.render();
        unsafe { self.gl.clear(glow::COLOR_BUFFER_BIT) };
        self.renderer.render(&self.gl, &mut self.textures, draw_data).unwrap();

        self.sdl_window.gl_swap_window();

        Ok(menu_event)
    }

    pub fn draw_game_screen(&mut self, screen: &[u8]) {
        let &texture = self.textures.get(self.game_window.texture_id()).unwrap();
        unsafe {
            self.gl.active_texture(texture.0.into());
            self.gl.texture_sub_image_2d(
                texture,
                0,
                0 as _,
                0 as _,
                GB_SCREEN_WIDTH as _,
                GB_SCREEN_HEIGHT as _,
                glow::RGBA as _,
                glow::UNSIGNED_BYTE,
                PixelUnpackData::Slice(screen)
            );
        }
    }
    
    pub fn draw_tiles(&mut self, tiles: &[u8]) {
        let &texture = self.textures.get(self.tile_window.texture_id()).unwrap();
        unsafe {
            self.gl.active_texture(texture.0.into());
            self.gl.texture_sub_image_2d(
                texture,
                0,
                0 as _,
                0 as _,
                (32 * 8) as _,
                (24 * 8) as _,
                glow::RGBA as _,
                glow::UNSIGNED_BYTE,
                PixelUnpackData::Slice(tiles)
            );
        }
    }
}

#[cfg(feature = "debug_ui")]
pub struct WaveformWindow {
    toggle: bool,
    ch_enable: (bool, bool, bool, bool)
}

#[cfg(feature = "debug_ui")]
impl WaveformWindow {
    pub fn new() -> Self {
        Self { toggle: false, ch_enable: (true, true, true, true) }
    }
    pub fn show(&mut self, gb: &mut GameBoy, ui: &mut Ui, pos: [f32; 2], audio: (&[f32], &[f32], &[f32], &[f32])) {
        if !self.toggle {
            return;
        }
        ui.window("Waveform")
            .position(pos, Condition::FirstUseEver)
            .size([400.0, 380.0], Condition::FirstUseEver)
            .build(|| {
                let sz = ui.content_region_avail();
                let wav_sz = [sz[0] - ui.calc_text_size("Enable Square 1")[0] - 40.0, sz[1] / 4.0 - ui.clone_style().frame_padding[1] * 2.0];
                //let wav_sz = [sz[0] / 4.0 * 3.0, sz[1] / 4.0 - ui.clone_style().frame_padding[1] * 2.0];
                ui.plot_lines("", audio.0)
                    .graph_size(wav_sz)
                    .scale_min(0.0)
                    .scale_max(1.0)
                    .overlay_text("Square 1")
                    .build();
                ui.same_line();
                if ui.checkbox("Enable Square 1", &mut self.ch_enable.0) {
                    gb.enable_audio_channel(0, self.ch_enable.0);
                }
                ui.separator();
                ui.plot_lines("", audio.1)
                    .graph_size(wav_sz)
                    .scale_min(0.0)
                    .scale_max(1.0)
                    .overlay_text("Square 2")
                    .build();
                ui.same_line();
                if ui.checkbox("Enable Square 2", &mut self.ch_enable.1) {
                    gb.enable_audio_channel(1, self.ch_enable.1);
                }
                ui.separator();
                ui.plot_lines("", audio.2)
                    .graph_size(wav_sz)
                    .scale_min(0.0)
                    .scale_max(1.0)
                    .overlay_text("Wave")
                    .build();
                ui.same_line();
                if ui.checkbox("Enable Wave", &mut self.ch_enable.2) {
                    gb.enable_audio_channel(2, self.ch_enable.2);
                }
                ui.separator();
                ui.plot_lines("", audio.3)
                    .graph_size(wav_sz)
                    .scale_min(0.0)
                    .scale_max(1.0)
                    .overlay_text("Noise")
                    .build();
                ui.same_line();
                if ui.checkbox("Enable Noise", &mut self.ch_enable.3) {
                    gb.enable_audio_channel(3, self.ch_enable.3);
                }
            });
    }

    pub fn toggle(&mut self) {
        self.toggle = !self.toggle;
    }
}

pub struct GameWindow {
    texture: TextureId,
}

impl GameWindow {
    pub fn new(texture: TextureId) -> Self {
        Self { texture }
    }

    fn game_screen(&self, ui: &mut Ui, screen_pos: [f32; 2], mut screen_size: [f32; 2], text: Option<String>) {
        let _a = ui.push_style_var(StyleVar::WindowPadding([0.0, 0.0]));
        let _b = ui.push_style_var(StyleVar::ChildBorderSize(0.0));

        let mut w = ui.window("ohBoi")
            .position(screen_pos, Condition::FirstUseEver)
            .size(screen_size, Condition::FirstUseEver);
        if cfg!(feature = "debug_ui") {
            w = w.movable(true).resizable(true).size(screen_size, Condition::FirstUseEver);
        } else {
            w = w.size(screen_size, Condition::Always).no_decoration();
        }

        w.draw_background(false)
            .build(|| {
                if let Some(t) = text {
                    ui.text(t);
                    screen_size[1] -= ui.text_line_height_with_spacing();
                }
                imgui::Image::new(self.texture, ui.content_region_avail()).build(ui);
            }).unwrap();
    }

    pub fn show(&self, ui: &mut Ui, sdl_window_size: (u32, u32), text: Option<String>) {
        let [_, imgui_menu_height] = ui.item_rect_size();
        let game_screen_size = if cfg!(feature = "debug_ui") {
            [(GB_SCREEN_WIDTH * 2) as f32, (GB_SCREEN_HEIGHT * 2) as f32]
        } else {
            [sdl_window_size.0 as f32, sdl_window_size.1 as f32 - imgui_menu_height]
        };

        self.game_screen(ui, [0.0, imgui_menu_height], game_screen_size, text);
    }

    pub fn texture_id(&self) -> TextureId {
        self.texture
    }
}

pub struct TileWindow {
    texture: TextureId
}

impl TileWindow {
    pub fn new(texture: TextureId) -> Self {
        Self { texture }
    }

    pub fn show(&self, ui: &mut Ui) {
        let _a = ui.push_style_var(StyleVar::WindowPadding([0.0, 0.0]));
        let _b = ui.push_style_var(StyleVar::ChildBorderSize(0.0));
        ui.window("Tile Viewer")
            .position([0.0, 0.0], Condition::FirstUseEver)
            .size([8.0 * 32.0, 8.0 * 12.0], Condition::FirstUseEver)
            .movable(true)
            .resizable(true)
            .draw_background(false)
            .build(|| {
                imgui::Image::new(self.texture, ui.content_region_avail()).build(ui);
            }).unwrap_or(());
    }
    
    pub fn texture_id(&self) -> TextureId {
        self.texture
    }
}