//! Terminal-based audio player with waveform visualization.
//!
//! The player module provides a complete audio playback experience in the terminal,
//! featuring real-time oscilloscope visualization, stereo LED meters, file browsing
//! with metadata search, mark/loop functionality, and audio export capabilities.
//! It's designed for fast sample browsing and editing workflows, allowing users to
//! quickly audition, select, and export portions of audio files.

pub mod app;
pub mod audio;
pub mod browser;
pub mod save_dialog;
pub mod save_dialog_ui;
pub mod telemetry;
pub mod ui;
pub mod waveform;

use std::error::Error;

pub fn run(pattern: Option<&str>, _interactive: bool) -> Result<(), Box<dyn Error>> {
    // Always launch TUI for now, but load file if provided
    app::run_with_file(pattern)
}
