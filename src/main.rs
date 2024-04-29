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
    let mut ui = OhBoiUi::new()?;
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
            Open(path) => gb.load_new_game(path)?,
            Close => break 'main,
            _ => {}
        }
    }

    Ok(())
}
