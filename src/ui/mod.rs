use std::any::Any;
use std::collections::HashMap;
use std::error::Error;
use std::io::Read;
use std::path::PathBuf;
use std::rc::Rc;
use imgui_glow_renderer::glow::PixelUnpackData;
use imgui::{Condition, StyleVar, TextureId, Textures, Ui, WindowFlags};
use imgui::Key::ModShortcut;
use imgui_glow_renderer::{AutoRenderer, Renderer};
use imgui_glow_renderer::glow::{Context, HasContext, Texture};
use imgui_sdl2_support::SdlPlatform;
use sdl2::{EventPump, VideoSubsystem};
use sdl2::video::{GLContext, Window, WindowContext};
use sdl2::event::{Event, EventType, EventWatch, EventWatchCallback, WindowEvent};
use sdl2::pixels::{Color, PixelFormatEnum};
use sdl2::render::{Canvas, WindowCanvas};

const WIDTH: usize = 160;
const HEIGHT: usize = 144;

pub struct GameWindow {
    canvas: Window,
    renderer: Renderer,
    texture: TextureId,
    window_size: (i32, i32),
    close: bool
}

impl GameWindow {
    pub fn new(canvas: Window, renderer: Renderer, texture: TextureId) -> Result<Self, Box<dyn Error>> {
        Ok(Self { canvas, renderer, texture, window_size: (160, 144), close: false })
    }

    pub fn should_close(&self) -> bool {
        self.close
    }

    pub fn handle_event(&mut self, e: &Event) -> bool {
        match e.get_window_id() {
            Some(id) if id == self.canvas.id() =>
            if let &Event::Window { win_event: WindowEvent::SizeChanged(w, h), .. } = e {
                self.window_size = (w, h);
            },
            _ => (),
        }

        false
    }

    fn main_menu_bar(&mut self, ui: &mut Ui) {
        if let Some(menubar) = ui.begin_main_menu_bar() {
            if let Some(menu) = ui.begin_menu("File") {
                ui.menu_item_config("Open").shortcut("Ctrl+O").build();
                self.close = ui.menu_item_config("Close").shortcut("Alt+F4").build();
                menu.end();
            }
            menubar.end();
        }
    }

    fn game_screen(&self, ui: &mut Ui, wsize: [f32; 2], pos: [f32; 2]) {
        let _a = ui.push_style_var(StyleVar::WindowPadding([0.0, 0.0]));
        let _b = ui.push_style_var(StyleVar::ChildBorderSize(0.0));

        ui.window("ohBoi")
            .position(pos, Condition::FirstUseEver)
            .size(wsize, Condition::Always)
            .draw_background(false)
            .no_decoration()
            .build(|| {
                imgui::Image::new(self.texture, wsize).build(ui);
            }).unwrap();
    }

    pub fn show(&mut self, gl: &Context, textures: &Textures<Texture>, platform: &mut SdlPlatform, imgui: &mut imgui::Context, event_pump: &EventPump)
        -> Result<(), Box<dyn Error>> {
        platform.prepare_frame(imgui, &self.canvas, event_pump);
        let ui = imgui.new_frame();

        self.main_menu_bar(ui);

        let [_, menu_height] = ui.item_rect_size();
        let wsize = [self.window_size.0 as f32, self.window_size.1 as f32 - menu_height];

        self.game_screen(ui, wsize, [0.0, menu_height]);

        let draw_data = imgui.render();

        unsafe { gl.clear(glow::COLOR_BUFFER_BIT) };
        self.renderer.render(gl, textures, draw_data)?;
        self.canvas.gl_swap_window();

        Ok(())
    }

    pub fn update_texture(&mut self, gl: &Context, textures: &Textures<Texture>, screen: &[u8]) {
        let &texture = textures.get(self.texture).unwrap();
        unsafe {
            gl.active_texture(texture);
            gl.tex_sub_image_2d(
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