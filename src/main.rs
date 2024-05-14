mod logging;
pub mod core;
mod ohboi;
mod ui;

use std::error::Error;
use std::fs::File;
use std::io::Write;
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
    let mut audio_queue = vec![0.0; 4096];
    let mut ch1_queue = vec![0.0; 2048];
    let mut ch2_queue = vec![0.0; 2048];
    let mut ch3_queue = vec![0.0; 2048];
    let mut ch4_queue = vec![0.0; 2048];
    
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
                    let (ch1, ch2, ch3, ch4) = gb.get_channels_output();
                    ch1_queue[buffer_pointer / 2] = ch1;
                    ch2_queue[buffer_pointer / 2] = ch2;
                    ch3_queue[buffer_pointer / 2] = ch3;
                    ch4_queue[buffer_pointer / 2] = ch4;
                    
                    audio_queue[buffer_pointer] = out.0;
                    buffer_pointer += 1;
                    audio_queue[buffer_pointer] = out.1;
                    buffer_pointer += 1;
                    if buffer_pointer == 4096 {
                        ui.audio_callback(&audio_queue);
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
        match ui.show(&mut gb, None, (&ch1_queue, &ch2_queue, &ch3_queue, &ch4_queue))? {
            Open(path) => {
                gb.close_game();
                gb.load_new_game(path)?;
            },
            Close => {
                gb.close_game();
                break 'main;
            },
            _ => {}
        }
        let elapsed = current_time.elapsed();
        fps = format!("FPS: {}", 1.0 / elapsed.as_secs_f64());
        if elapsed.as_secs_f64() < 1.0 / 60.0 {
            thread::sleep(std::time::Duration::from_secs_f64(1.0 / 60.0) - elapsed);
        }
    }

    Ok(())
}
