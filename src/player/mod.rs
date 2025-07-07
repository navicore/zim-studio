pub mod app;
pub mod audio;
pub mod browser;
pub mod ui;
pub mod waveform;

use std::error::Error;

pub fn run(pattern: Option<&str>, _interactive: bool) -> Result<(), Box<dyn Error>> {
    // Always launch TUI for now, but load file if provided
    app::run_with_file(pattern)
}
