use std::error::Error;
use std::path::PathBuf;

use imgui_glow_renderer::glow::{NativeTexture, PixelUnpackData};
use imgui::{Condition, StyleVar, TextureId, Textures, Ui};
use imgui_glow_renderer::Renderer;
use imgui_glow_renderer::glow::{Context, HasContext, Texture};
use imgui_sdl2_support::SdlPlatform;
use sdl2::{EventPump, Sdl};
use sdl2::video::{GLContext, GLProfile, Window};
use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use crate::core::GameBoy;
use crate::core::joypad::Key;
use crate::ui::GameWindowEvent::{Close, Nothing, Open};

const GB_SCREEN_WIDTH: usize = 160;
const GB_SCREEN_HEIGHT: usize = 144;

pub enum GameWindowEvent {
    Close,
    Open(PathBuf),
    KeyPress(Keycode),
    Nothing
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

fn menu_bar_event_handler(gb: &mut GameBoy, e: &GameWindowEvent) -> Result<bool, Box<dyn Error>> {
    match e {
        Open(path) => match gb.load_new_game(path.clone()) {
            Ok(_) => Ok(false),
            Err(e) => Err(Box::try_from(e).unwrap())
        },
        Close => Ok(true),
        _ => Ok(false)
    }
}

fn sdl_event_handler(e: &Event, gb: &mut GameBoy) -> Result<bool, Box<dyn Error>> {
    match e {
        Event::KeyDown { keycode: Some(k), .. } => {
            match k {
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
            match k {
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
    textures: Textures<Texture>
}

impl OhBoiUi {
    pub fn new()
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

        let platform = SdlPlatform::init(&mut imgui);

        let video_subsystem = sdl.video()?;
        let sdl_window =
            video_subsystem.window("ohboi", GB_SCREEN_WIDTH as u32, GB_SCREEN_HEIGHT as u32)
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
        let renderer = Renderer::initialize(&gl, &mut imgui, &mut textures, false)?;
        let game_window = GameWindow::new(gb_screen_texture);

        Ok(Self { sdl, gl, gl_context, imgui, platform, sdl_window, renderer, game_window, textures })
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
                                                          Some((&["*.gb"], "Gameboy ROMs")))
                    {
                        return Open(PathBuf::from(path));
                    }
                }
                if ui.menu_item_config("Close").shortcut("Alt+F4").build() {
                    return Close;
                }
                menu.end();
            }
            menubar.end();
        }

        Nothing
    }

    pub fn show(&mut self, gb: &mut GameBoy) -> Result<GameWindowEvent, Box<dyn Error>> {
        let quit = self.process_sdl_events(gb)?;
        if quit {
            return Ok(Close);
        }
        let event_pump = self.sdl.event_pump()?;

        self.platform.prepare_frame(&mut self.imgui, &self.sdl_window, &event_pump);
        let ui = self.imgui.new_frame();

        let menu_event = Self::main_menu_bar(ui);
        self.game_window.show(ui, self.sdl_window.size(), None);

        let draw_data = self.imgui.render();
        unsafe { self.gl.clear(glow::COLOR_BUFFER_BIT) };
        self.renderer.render(&self.gl, &mut self.textures, draw_data).unwrap();

        self.sdl_window.gl_swap_window();

        menu_bar_event_handler(gb, &menu_event)?;

        Ok(menu_event)
    }

    pub fn draw_game_screen(&mut self, screen: &[u8]) {
        let &texture = self.textures.get(self.game_window.texture_id()).unwrap();
        unsafe {
            self.gl.active_texture(texture.0.into());
            self.gl.tex_sub_image_2d(
                glow::TEXTURE_2D,
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
}

pub struct GameWindow {
    texture: TextureId,
}

impl GameWindow {
    pub fn new(texture: TextureId) -> Self {
        Self { texture }
    }

    fn game_screen(&self, ui: &mut Ui, screen_pos: [f32; 2], mut screen_size: [f32; 2], text: Option<&str>) {
        let _a = ui.push_style_var(StyleVar::WindowPadding([0.0, 0.0]));
        let _b = ui.push_style_var(StyleVar::ChildBorderSize(0.0));

        ui.window("ohBoi")
            .position(screen_pos, Condition::FirstUseEver)
            .size(screen_size, Condition::Always)
            .movable(false)
            .draw_background(false)
            .no_decoration()
            .build(|| {
                if let Some(t) = text {
                    ui.text(t);
                    screen_size[1] -= ui.text_line_height_with_spacing();
                }
                imgui::Image::new(self.texture, screen_size).build(ui);
            }).unwrap();
    }

    pub fn show(&self, ui: &mut Ui, sdl_window_size: (u32, u32), text: Option<&str>) {
        let [_, imgui_menu_height] = ui.item_rect_size();
        let game_screen_size = [sdl_window_size.0 as f32, sdl_window_size.1 as f32 - imgui_menu_height];
        self.game_screen(ui, [0.0, imgui_menu_height], game_screen_size, text);
    }

    pub fn texture_id(&self) -> TextureId {
        self.texture
    }
}