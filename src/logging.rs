use fern::colors::{Color, ColoredLevelConfig};

pub fn setup_logger(verbosity: u64, cpu_verbosity: u64) -> Result<(), fern::InitError> {
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
            out.finish(format_args!(
                "{color_line}[{target}:{level}] {message}\x1B[0m",
                color_line = format_args!("\x1B[{}m", colors_line.get_color(&record.level()).to_fg_str()),
                target = record.target(),
                level = record.level(),
                message = message
            ));
        })
        .chain(std::io::stdout())
        .apply()?;
    Ok(())
}