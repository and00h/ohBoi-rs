mod logging;
pub mod core;
mod ohboi;
mod ui;

use std::error::Error;
use std::path::PathBuf;
use std::thread;
use std::thread::JoinHandle;
use imgui_glow_renderer::{Renderer};
use imgui_glow_renderer::glow::HasContext;
use imgui_sdl2_support::SdlPlatform;
use log::{info, error, warn, debug, trace};
use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::video::GLProfile;
use crate::core::GameBoy;
use crate::core::joypad::Key;
use crate::ui::{GameWindow, OhBoiUi};
use crate::ui::GameWindowEvent::*;

fn main() -> Result<(), Box<dyn Error>> {
    logging::setup_logger(0, 0)?;
    info!("Starting ohBoi");
    let mut gb = GameBoy::new(PathBuf::from("./tetris.gb"))?;
    let mut ui = OhBoiUi::new(|e, gb| {
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
            _ => {}
        };
        Ok(())
    })?;
    'main: loop {
        let mut rendered = false;
        while gb.cycle_counter() < 4194304 / 60 {
            gb.clock();
            if !rendered && gb.is_in_vblank() {
                ui.draw_game_screen(&gb.screen());
                rendered = true;
            }
        }
        gb.reset_cycle_counter();
        match ui.show(&mut gb)? {
            Open(path) => { gb = GameBoy::new(path)?; },
            Close => break 'main,
            _ => {}
        }
    }

    Ok(())
}
