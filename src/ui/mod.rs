use std::collections::HashMap;
use std::error::Error;
use std::io::Read;
use std::rc::Rc;
use imgui_glow_renderer::glow::PixelUnpackData;
use imgui::{Condition, StyleVar, TextureId, Textures, WindowFlags};
use imgui_glow_renderer::{AutoRenderer, Renderer};
use imgui_glow_renderer::glow::{Context, HasContext, Texture};
use imgui_sdl2_support::SdlPlatform;
use sdl2::{EventPump, VideoSubsystem};
use sdl2::video::{GLContext, Window, WindowContext};
use sdl2::event::Event;
use sdl2::pixels::{Color, PixelFormatEnum};
use sdl2::render::{Canvas, WindowCanvas};

const WIDTH: usize = 160;
const HEIGHT: usize = 144;

pub struct GameWindow {
    canvas: Window,
    renderer: Renderer,
    texture: TextureId,
    counter: u8
}

impl GameWindow {
    pub fn new(canvas: Window, renderer: Renderer, texture: TextureId, width: u32, height: u32) -> Result<Self, Box<dyn Error>> {
        Ok(Self { canvas, renderer, texture, counter: 0 })
    }

    pub fn handle_event(&mut self, e: &Event) -> bool {
        match e.get_window_id() {
            Some(id) if id == self.canvas.id() =>
                matches!(e, &Event::Quit { .. }),
            _ => false,
        }
    }

    pub fn show(&mut self, gl: &Context, textures: &Textures<Texture>, platform: &mut SdlPlatform, imgui: &mut imgui::Context, event_pump: &EventPump)
        -> Result<(), Box<dyn Error>> {
        platform.prepare_frame(imgui, &self.canvas, event_pump);
        let ui = imgui.new_frame();
        if let Some(menubar) = ui.begin_main_menu_bar() {
            if let Some(menu) = ui.begin_menu("File") {
                ui.menu_item("Open");
                menu.end();
            }
            menubar.end();
        }
        let [_, y] = ui.item_rect_size();
        let wsize = self.canvas.size();
        let wsize = [wsize.0 as f32, wsize.1 as f32 - y];

        let a = ui.push_style_var(StyleVar::WindowPadding([0.0, 0.0]));
        let b = ui.push_style_var(StyleVar::ChildBorderSize(0.0));

        ui.window("ohboi")
            .position([0.0, y], Condition::FirstUseEver)
            .size(wsize, Condition::Always)
            .draw_background(false)
            .no_decoration()
            .build(|| {
                imgui::Image::new(self.texture, wsize).build(ui);
            }).unwrap();
        a.end();
        b.end();

        let draw_data = imgui.render();
        unsafe { gl.clear(glow::COLOR_BUFFER_BIT) };
        self.renderer.render(gl, textures, draw_data);
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