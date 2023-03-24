mod logging;
mod core;

use std::error::Error;
use std::path::PathBuf;
use log::{info, error, warn, debug, trace};
use crate::core::GameBoy;

fn main() -> Result<(), Box<dyn Error>> {
    logging::setup_logger(0)?;
    info!("Starting ohBoi");

    Ok(())
}
