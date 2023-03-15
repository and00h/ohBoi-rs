mod core;
mod logging;

use std::error::Error;
use log::{info, error, warn, debug, trace};

fn main() -> Result<(), Box<dyn Error>> {
    logging::setup_logger(4)?;
    info!("Starting ohBoi");

    Ok(())
}
