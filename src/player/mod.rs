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
pub mod mixed_source;
pub mod save_dialog;
pub mod save_dialog_ui;
pub mod telemetry;
pub mod ui;
pub mod waveform;

use std::error::Error;

pub fn run(
    files: Vec<String>,
    gains: Option<Vec<f32>>,
    _interactive: bool,
) -> Result<(), Box<dyn Error>> {
    // Always launch TUI for now, but load file(s) if provided
    if files.is_empty() {
        app::run_with_file(None, None)
    } else if files.len() == 1 {
        // Single file playback
        app::run_with_file(Some(&files[0]), None)
    } else if gains.is_some() {
        // Multiple files with gains specified - mixing mode (simultaneous playback)
        app::run_with_files(&files, gains)
    } else {
        // Multiple files without gains - playlist mode (sequential playback)
        app::run_with_playlist(&files)
    }
}
