mod logging;
pub mod core;
mod ohboi;
mod ui;

use std::error::Error;
use std::path::PathBuf;
use imgui_glow_renderer::{Renderer};
use imgui_glow_renderer::glow::HasContext;
use imgui_sdl2_support::SdlPlatform;
use log::{info, error, warn, debug, trace};
use sdl2::event::Event;
use sdl2::video::GLProfile;
use crate::core::GameBoy;
use crate::ui::GameWindow;

fn main() -> Result<(), Box<dyn Error>> {
    logging::setup_logger(0, 0)?;
    info!("Starting ohBoi");
    let mut gb = GameBoy::new(PathBuf::from(".\\dmg-acid2.gb"))?;
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

    let mut platform = SdlPlatform::init(&mut imgui);
    let window =
        video_subsystem.window("ohboi", 160, 144)
            .allow_highdpi()
            .opengl()
            .resizable()
            .position_centered()
            .build()?;
    let context = window.gl_create_context()?;
    window.gl_make_current(&context)?;
    let gl = unsafe {
        imgui_glow_renderer::glow::Context::from_loader_function(|s| video_subsystem.gl_get_proc_address(s) as _)
    };
    window.subsystem().gl_set_swap_interval(1).unwrap();
    let mut textures = imgui::Textures::<imgui_glow_renderer::glow::Texture>::new();
    const WIDTH: usize = 160;
    const HEIGHT: usize = 144;

    let gl_texture = unsafe { gl.create_texture() }.expect("unable to create GL texture");

    unsafe {
        gl.bind_texture(glow::TEXTURE_2D, Some(gl_texture));
        gl.tex_parameter_i32(
            glow::TEXTURE_2D,
            glow::TEXTURE_MIN_FILTER,
            glow::LINEAR as _,
        );
        gl.tex_parameter_i32(
            glow::TEXTURE_2D,
            glow::TEXTURE_MAG_FILTER,
            glow::NEAREST as _,
        );
        gl.tex_image_2d(
            glow::TEXTURE_2D,
            0,
            glow::RGBA as _, // When generating a texture like this, you're probably working in linear color space
            WIDTH as _,
            HEIGHT as _,
            0,
            glow::RGBA,
            glow::UNSIGNED_BYTE,
            None,
        )
    }

    let id = textures.insert(gl_texture);
    let renderer = Renderer::initialize(&gl, &mut imgui, &mut textures, false)?;

    let mut window = GameWindow::new(window, renderer, id, 160, 144)?;
    let mut event_pump = sdl.event_pump()?;
    'main: loop {
        for event in event_pump.poll_iter() {
            platform.handle_event(&mut imgui, &event);
            window.handle_event(&event);
            if matches!(event, Event::Quit { .. }) {
                break 'main;
            }
        }

        let mut rendered = false;
        while gb.cycle_counter() < 4194304 / 60 {
            gb.clock();
            if !rendered && gb.is_in_vblank() {
                window.update_texture(&gl, &textures, &gb.screen());
                rendered = true;
            }
        }
        gb.reset_cycle_counter();
        window.show(&gl, &textures, &mut platform, &mut imgui, &event_pump)?;
    }

    Ok(())
}
