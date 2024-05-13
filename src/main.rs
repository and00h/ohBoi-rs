#![feature(iter_array_chunks)]

mod logging;
pub mod core;
mod ohboi;
mod ui;

use std::error::Error;
use std::fs::File;
use std::path::PathBuf;
use std::thread;
use log::{info};
use crate::core::GameBoy;
use crate::logging::setup_logger;
use crate::ui::{OhBoiUi};
use crate::ui::GameWindowEvent::*;

fn main() -> Result<(), Box<dyn Error>> {
    logging::setup_logger(2, 0)?;
    info!("Starting ohBoi");
    let mut gb = GameBoy::new(PathBuf::from("./tetris.gb"))?;
    let mut ui = OhBoiUi::new()?;
    let mut audio_queue = vec![0.0; 2048];
    let mut buffer_pointer = 0;
    'main: loop {
        let current_time = std::time::Instant::now();
        let mut fps = String::from("0.0");
        let mut rendered = false;
        let mut speed = 1;
        while gb.cycle_counter() < 4194304 / 60 * speed {
            gb.clock();
            match gb.audio_output() {
                Some(out) => {
                    audio_queue[buffer_pointer] = out.0;
                    buffer_pointer += 1;
                    audio_queue[buffer_pointer] = out.1;
                    buffer_pointer += 1;
                    if buffer_pointer == 2048 {
                        let res = audio_queue.iter()
                            .array_chunks::<2>()
                            .step_by(speed as usize)
                            .flatten()
                            .copied()
                            .collect::<Vec<f32>>();
                        ui.audio_callback(&res);
                        buffer_pointer = 0;
                    }
                }
                None => {}
            }
            if !rendered && gb.is_in_vblank() {
                ui.draw_game_screen(&gb.screen());
                rendered = true;
            }
        }
        gb.reset_cycle_counter();
        match ui.show(&mut gb, Some(fps))? {
            Open(path) => gb.load_new_game(path)?,
            Close => break 'main,
            _ => {}
        }
        let elapsed = current_time.elapsed();
        fps = format!("{}", 1.0 / elapsed.as_secs_f64() * 100.0);
    }

    Ok(())
}
