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
use crate::ui::GameWindowEvent::{Close, Nothing, Open};

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

pub struct OhBoiUi<'a> {
    sdl: Sdl,
    imgui: imgui::Context,
    platform: SdlPlatform,
    game_window: GameWindow<'a>,
    textures: Textures<Texture>
}

impl<'a> OhBoiUi<'a> {
    pub fn new<F: FnMut(&Event, &mut GameBoy) -> Result<(), Box<dyn Error>> + 'a>(event_handler: F) -> Result<Self, Box<dyn Error>> {
        let sdl = sdl2::init()?;
        let video_subsystem = sdl.video()?;
        let gl_attr = video_subsystem.gl_attr();
        gl_attr.set_context_version(3, 3);
        gl_attr.set_context_profile(GLProfile::Core);

        let mut imgui = imgui::Context::create();
        imgui.set_ini_filename(None);
        imgui.set_log_filename(None);
        imgui.style_mut().display_window_padding = [0.0, 0.0];
        imgui.fonts().add_font(&[imgui::FontSource::DefaultFontData { config: None }]);

        let mut textures = Textures::<Texture>::new();
        let game_window = GameWindow::new(&sdl, &mut imgui, &mut textures, event_handler)?;
        let platform = SdlPlatform::init(&mut imgui);

        Ok(Self { sdl, imgui, platform, game_window, textures })
    }

    fn process_sdl_events(&mut self, gb: &mut GameBoy) -> Result<(), Box<dyn Error>> {
        for event in self.sdl.event_pump()?.poll_iter() {
            self.platform.handle_event(&mut self.imgui, &event);
            self.game_window.handle_event(&event, gb)?;
        }

        Ok(())
    }

    pub fn show(&mut self, gb: &mut GameBoy) -> Result<GameWindowEvent, Box<dyn Error>> {
        self.process_sdl_events(gb)?;
        let event_pump = self.sdl.event_pump()?;
        let event = self.game_window.show(&self.textures, &mut self.platform, &mut self.imgui, &event_pump, None)?;
        Ok(event)
    }

    pub fn draw_game_screen(&mut self, screen: &[u8]) {
        self.game_window.update_texture(&self.textures, screen);
    }
}

const WIDTH: usize = 160;
const HEIGHT: usize = 144;

pub enum GameWindowEvent {
    Close,
    Open(PathBuf),
    KeyPress(Keycode),
    Nothing
}

pub struct GameWindow<'a> {
    context: GLContext,
    gl: Context,
    window: Window,
    renderer: Renderer,
    texture: TextureId,
    event_handler: Box<dyn FnMut(&Event, &mut GameBoy) -> Result<(), Box<dyn Error>> + 'a>
}

impl<'a> GameWindow<'a> {
    pub fn new<F: FnMut(&Event, &mut GameBoy) -> Result<(), Box<dyn Error>> + 'a>(sdl: &Sdl, imgui: &mut imgui::Context, textures: &mut Textures<NativeTexture>, event_handler: F) -> Result<Self, Box<dyn Error>> {
        let video_subsystem = sdl.video()?;
        let window =
            video_subsystem.window("ohboi", 160, 144)
                .allow_highdpi()
                .opengl()
                .resizable()
                .position_centered()
                .build()?;
        let context = window.gl_create_context()?;
        window.gl_make_current(&context)?;
        window.subsystem().gl_set_swap_interval(1)?;
        let gl = unsafe {
            Context::from_loader_function(|s| video_subsystem.gl_get_proc_address(s) as _)
        };
        let texture = new_texture(160, 144, &gl, textures)?;
        let renderer = Renderer::initialize(&gl, imgui, textures, false)?;

        Ok(Self { context, gl, window, renderer, texture, event_handler: Box::new(event_handler) })
    }

    pub fn handle_event(&mut self, e: &Event, gb: &mut GameBoy) -> Result<(), Box<dyn Error>> {
        match e.get_window_id() {
            Some(id) if id == self.window.id() => (self.event_handler)(e, gb),
            _ => Ok(())
        }
    }

    fn main_menu_bar(&self, ui: &mut Ui) -> GameWindowEvent {
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

    pub fn show(&mut self, textures: &Textures<Texture>, platform: &mut SdlPlatform, imgui: &mut imgui::Context, event_pump: &EventPump, text: Option<&str>)
        -> Result<GameWindowEvent, Box<dyn Error>> {
        platform.prepare_frame(imgui, &self.window, event_pump);
        let ui = imgui.new_frame();

        let menu_event = Ok(self.main_menu_bar(ui));

        let [_, imgui_menu_height] = ui.item_rect_size();

        let sdl_window_size = self.window.size();
        let game_screen_size = [sdl_window_size.0 as f32, sdl_window_size.1 as f32 - imgui_menu_height];

        self.game_screen(ui, [0.0, imgui_menu_height], game_screen_size, text);

        let draw_data = imgui.render();
        unsafe { self.gl.clear(glow::COLOR_BUFFER_BIT) };
        self.renderer.render(&self.gl, textures, draw_data).unwrap();
        self.window.gl_swap_window();

        menu_event
    }

    pub fn update_texture(&mut self, textures: &Textures<Texture>, screen: &[u8]) {
        let &texture = textures.get(self.texture).unwrap();
        unsafe {
            self.gl.active_texture(texture.0.into());
            self.gl.tex_sub_image_2d(
                glow::TEXTURE_2D,
                0,
                0 as _,
                0 as _,
                160 as _,
                144 as _,
                glow::RGBA as _,
                glow::UNSIGNED_BYTE,
                PixelUnpackData::Slice(screen)
            );
        }
    }
}