use std::sync::{Arc, Mutex};
use fern::colors::{Color, ColoredLevelConfig, WithFgColor};
use regex::{Regex, RegexBuilder};

pub struct ImguiLogString {
    pub text: String,
    pub level: log::Level
}

pub fn setup_logger(verbosity: u64, cpu_verbosity: u64, log_buffer: Arc<Mutex<Vec<ImguiLogString>>>) -> Result<(), fern::InitError> {
    let mut config = fern::Dispatch::new();
    config = match verbosity {
        0 => config.level(log::LevelFilter::Off),
        1 => config.level(log::LevelFilter::Warn),
        2 => config.level(log::LevelFilter::Info),
        3 => config.level(log::LevelFilter::Debug),
        _ => config.level(log::LevelFilter::Trace)
    };
    config = match cpu_verbosity {
        0 => config.level_for("ohBoi_rs::core::cpu", log::LevelFilter::Off),
        1 => config.level_for("ohBoi_rs::core::cpu", log::LevelFilter::Warn),
        2 => config.level_for("ohBoi_rs::core::cpu", log::LevelFilter::Info),
        3 => config.level_for("ohBoi_rs::core::cpu", log::LevelFilter::Debug),
        _ => config.level_for("ohBoi_rs::core::cpu", log::LevelFilter::Trace)
    };

    let colors_line = ColoredLevelConfig::new()
        .error(Color::Red)
        .warn(Color::Yellow)
        .info(Color::Cyan)
        .debug(Color::Green)
        .trace(Color::Magenta);
    config
        .format(move |out, message, record| {
            if cfg!(unix) && !cfg!(feature = "debug_ui") {
                out.finish(format_args!(
                    "[{level}] [{target}] {message}",
                    target = record.target(),
                    level = colors_line.color(record.level()),
                    message = message
                ));
            } else { 
                out.finish(format_args!(
                    "[{level}] [{target}] {message}",
                    target = record.target(),
                    level = record.level(),
                    message = message
                ));
            };
        })
        .chain(std::io::stdout())
        .chain(fern::Output::call(move |record| {
            log_buffer.lock().unwrap().push(ImguiLogString { text: record.args().to_string(), level: record.level() });
        }))
        .apply()?;
    Ok(())
}