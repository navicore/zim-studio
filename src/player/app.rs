//! Main application state and control flow for the audio player.
//!
//! This module serves as the central coordinator for the terminal-based audio player,
//! managing the interaction between the audio engine, user interface, file browser,
//! and save functionality. It handles the main event loop, keyboard input processing,
//! and state transitions between different modes (playback, browsing, saving).

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use log::info;
use ratatui::{Terminal, backend::CrosstermBackend};
use std::{error::Error, io, path::PathBuf, time::Duration};

use super::audio::AudioEngine;
use super::browser::Browser;
use super::save_dialog::SaveDialog;
use super::telemetry::{AudioTelemetry, TelemetryConfig};
use super::ui;
use super::waveform::WaveformBuffer;
use std::sync::mpsc;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ViewMode {
    Player,
    Browser,
}

pub struct App {
    pub should_quit: bool,
    pub current_file: Option<String>,
    pub is_playing: bool,
    pub audio_engine: Option<AudioEngine>,
    pub waveform_buffer: WaveformBuffer,
    samples_rx: Option<mpsc::Receiver<Vec<f32>>>,
    pub left_level: f32,
    pub right_level: f32,
    pub is_stereo: bool,
    pub browser: Browser,
    pub playback_position: f32, // 0.0 to 1.0
    pub duration: Option<std::time::Duration>,
    pub mark_in: Option<f32>,  // 0.0 to 1.0
    pub mark_out: Option<f32>, // 0.0 to 1.0
    edit_counter: u32,         // Track number of edits this session
    pub save_dialog: Option<SaveDialog>,
    pub is_looping: bool, // Whether we're looping the selection
    pub view_mode: ViewMode,
    pub telemetry: AudioTelemetry,
    previous_left_level: f32,           // For slew gate rate calculation
    previous_right_level: f32,          // For slew gate rate calculation
    pub editor_message: Option<String>, // Message to show when editor can't open
    editor_message_timer: Option<std::time::Instant>, // When to clear the message
}

impl App {
    pub fn new() -> Self {
        Self {
            should_quit: false,
            current_file: None,
            is_playing: false,
            audio_engine: None,
            waveform_buffer: WaveformBuffer::new(4096),
            samples_rx: None,
            left_level: 0.0,
            right_level: 0.0,
            is_stereo: false,
            browser: Browser::new(),
            playback_position: 0.0,
            duration: None,
            mark_in: None,
            mark_out: None,
            edit_counter: 0,
            save_dialog: None,
            is_looping: false,
            view_mode: ViewMode::Player,
            telemetry: AudioTelemetry::new(),
            previous_left_level: 0.0,
            previous_right_level: 0.0,
            editor_message: None,
            editor_message_timer: None,
        }
    }

    pub fn load_file(&mut self, path: &str) -> Result<(), Box<dyn Error>> {
        // Create audio engine if needed
        if self.audio_engine.is_none() {
            let (engine, samples_rx) = AudioEngine::new()?;
            self.audio_engine = Some(engine);
            self.samples_rx = Some(samples_rx);
        }

        // Load the file
        if let Some(engine) = &mut self.audio_engine {
            engine.load_file(std::path::Path::new(path))?;

            // Update channel info and duration
            if let Some(info) = &engine.info {
                self.is_stereo = info.channels > 1;
            }
            self.duration = engine.duration;

            self.current_file = Some(path.to_string());

            // Start playback automatically when file is loaded
            self.is_playing = true;
            engine.play();
        }

        Ok(())
    }

    pub fn load_files(
        &mut self,
        paths: &[String],
        gains: Option<Vec<f32>>,
    ) -> Result<(), Box<dyn Error>> {
        // Create audio engine if needed
        if self.audio_engine.is_none() {
            let (engine, samples_rx) = AudioEngine::new()?;
            self.audio_engine = Some(engine);
            self.samples_rx = Some(samples_rx);
        }

        // Load the files for mixing
        if let Some(engine) = &mut self.audio_engine {
            engine.load_files(paths, gains)?;

            // Update channel info and duration
            if let Some(info) = &engine.info {
                self.is_stereo = info.channels > 1;
            }
            self.duration = engine.duration;

            // Store files info for display
            let display_name = if paths.len() == 1 {
                paths[0].clone()
            } else {
                format!("Mixing {} files", paths.len())
            };
            self.current_file = Some(display_name);

            // Start playback automatically when files are loaded
            self.is_playing = true;
            engine.play();
        }

        Ok(())
    }

    /// Enable telemetry with custom configuration for debugging slew gates and VC control
    #[allow(dead_code)]
    pub fn enable_telemetry(&mut self, config: TelemetryConfig) {
        self.telemetry.update_config(config);
    }

    /// Enable telemetry with default debugging configuration
    pub fn enable_debug_telemetry(&mut self) {
        let config = TelemetryConfig {
            enabled: true,
            debug_audio_levels: true,
            debug_format_info: true,
            capture_interval_ms: 100, // 10Hz for debugging
            output_format: "log".to_string(),
            ..Default::default()
        };
        self.telemetry.update_config(config);
        log::info!("Audio telemetry enabled for slew gate and VC control debugging");
    }

    /// Disable telemetry
    pub fn disable_telemetry(&mut self) {
        let mut config = self.telemetry.config().clone();
        config.enabled = false;
        self.telemetry.update_config(config);
    }

    /// Export telemetry data
    #[allow(dead_code)]
    pub fn export_telemetry(&self, format: &str) -> Result<String, Box<dyn Error>> {
        match format {
            "json" => Ok(self.telemetry.export_json()?),
            "csv" => Ok(self.telemetry.export_csv()),
            _ => Err("Unsupported export format. Use 'json' or 'csv'".into()),
        }
    }

    pub fn toggle_playback(&mut self) {
        if let Some(engine) = &mut self.audio_engine {
            if self.is_playing {
                engine.pause();
                self.is_playing = false;
            } else {
                // If at 100%, restart from beginning
                if self.playback_position >= 0.99 {
                    let _ = engine.seek_relative(-self.duration.unwrap_or_default().as_secs_f32());
                }
                engine.play();
                self.is_playing = true;
            }
        }
    }

    pub fn update_waveform(&mut self) {
        self.process_audio_samples();
        self.update_playback_state();
        self.apply_level_decay();
    }

    fn process_audio_samples(&mut self) {
        let mut samples_to_process = Vec::new();

        if let Some(rx) = &self.samples_rx {
            while let Ok(samples) = rx.try_recv() {
                self.waveform_buffer.push_samples(&samples);
                if !samples.is_empty() {
                    samples_to_process.push(samples);
                }
            }
        }

        for samples in samples_to_process {
            self.calculate_audio_levels(&samples);
        }
    }

    fn calculate_audio_levels(&mut self, samples: &[f32]) {
        // Store previous levels for slew gate calculation
        self.previous_left_level = self.left_level;
        self.previous_right_level = self.right_level;

        if self.is_stereo {
            self.calculate_stereo_levels(samples);
        } else {
            self.calculate_mono_levels(samples);
        }

        // Apply gentler decay for better visibility
        self.left_level = (self.left_level * 0.98).max(self.left_level * 0.8);
        self.right_level = (self.right_level * 0.98).max(self.right_level * 0.8);

        // Capture telemetry data for observability
        let playback_state = if self.is_playing {
            "playing"
        } else {
            "stopped"
        };
        let audio_info = self.audio_engine.as_ref().and_then(|e| e.info.as_ref());

        self.telemetry.maybe_capture(
            self.left_level,
            self.right_level,
            self.previous_left_level,
            self.previous_right_level,
            playback_state,
            self.playback_position,
            Some(samples),
            audio_info,
        );
    }

    fn calculate_stereo_levels(&mut self, samples: &[f32]) {
        let mut left_sum = 0.0;
        let mut right_sum = 0.0;
        let mut left_count = 0;
        let mut right_count = 0;

        for (i, &sample) in samples.iter().enumerate() {
            if i % 2 == 0 {
                left_sum += sample * sample;
                left_count += 1;
            } else {
                right_sum += sample * sample;
                right_count += 1;
            }
        }

        // Amplify the RMS values to make LEDs more responsive
        self.left_level = ((left_sum / left_count.max(1) as f32).sqrt() * 2.0).min(1.0);
        self.right_level = ((right_sum / right_count.max(1) as f32).sqrt() * 2.0).min(1.0);
    }

    fn calculate_mono_levels(&mut self, samples: &[f32]) {
        let sum: f32 = samples.iter().map(|s| s * s).sum();
        // Mono - amplify for better visibility
        let rms = ((sum / samples.len() as f32).sqrt() * 2.0).min(1.0);
        self.left_level = rms;
        self.right_level = rms;
    }

    fn update_playback_state(&mut self) {
        let mut need_loop_seek = None;
        let mut should_stop = false;

        if let Some(engine) = &self.audio_engine {
            self.playback_position = engine.get_progress();

            // Check if we've reached the end (not looping)
            if !self.is_looping && self.is_playing && self.playback_position >= 1.0 {
                should_stop = true;
            }

            // Check if we need to loop
            if self.is_looping && self.is_playing {
                need_loop_seek = self.check_loop_boundaries();
            }
        }

        // Stop playback if we've reached the end
        if should_stop {
            self.is_playing = false;
            if let Some(engine) = &self.audio_engine {
                engine.pause();
            }
        }

        // Apply loop seek if needed
        if let (Some(offset), Some(engine)) = (need_loop_seek, &mut self.audio_engine) {
            let _ = engine.seek_relative(offset);
        }
    }

    fn check_loop_boundaries(&self) -> Option<f32> {
        if let (Some(mark_in), Some(mark_out)) = (self.mark_in, self.mark_out) {
            let loop_start = mark_in.min(mark_out);
            let loop_end = mark_in.max(mark_out);

            // Check if we've reached the end of the loop or are before the start
            if (self.playback_position >= loop_end || self.playback_position < loop_start)
                && let Some(duration) = self.duration
            {
                let current_seconds = duration.as_secs_f32() * self.playback_position;
                let start_seconds = duration.as_secs_f32() * loop_start;
                return Some(start_seconds - current_seconds);
            }
        }
        None
    }

    fn apply_level_decay(&mut self) {
        // Store previous levels for slew gate calculation
        self.previous_left_level = self.left_level;
        self.previous_right_level = self.right_level;

        if self.is_playing {
            self.left_level *= 0.99; // Slower decay for better visibility
            self.right_level *= 0.99;
        } else {
            self.left_level = 0.0;
            self.right_level = 0.0;
        }

        // Capture telemetry for decay behavior (slew gate monitoring)
        let playback_state = if self.is_playing {
            "playing"
        } else {
            "stopped"
        };
        let audio_info = self.audio_engine.as_ref().and_then(|e| e.info.as_ref());

        self.telemetry.maybe_capture(
            self.left_level,
            self.right_level,
            self.previous_left_level,
            self.previous_right_level,
            playback_state,
            self.playback_position,
            None, // No sample data during decay
            audio_info,
        );
    }

    pub fn set_mark_in(&mut self) {
        self.mark_in = Some(self.playback_position);
        info!("Mark in set at {:.1}%", self.playback_position * 100.0);
    }

    pub fn set_mark_out(&mut self) {
        self.mark_out = Some(self.playback_position);
        info!("Mark out set at {:.1}%", self.playback_position * 100.0);
    }

    pub fn clear_marks(&mut self) {
        self.mark_in = None;
        self.mark_out = None;
        self.is_looping = false; // Stop looping when marks are cleared
        info!("Marks cleared");
    }

    pub fn toggle_loop(&mut self) {
        if self.mark_in.is_some() && self.mark_out.is_some() {
            self.is_looping = !self.is_looping;
            info!(
                "Loop {}",
                if self.is_looping {
                    "enabled"
                } else {
                    "disabled"
                }
            );

            // If starting loop, jump to mark in position
            if self.is_looping
                && let (Some(mark_in), Some(duration), Some(engine)) =
                    (self.mark_in, self.duration, &mut self.audio_engine)
            {
                let start_seconds = duration.as_secs_f32() * mark_in;
                let current_seconds = duration.as_secs_f32() * self.playback_position;
                let offset = start_seconds - current_seconds;
                let _ = engine.seek_relative(offset);
            }
        } else {
            info!("Cannot loop without both marks set");
        }
    }

    pub fn get_selection_duration(&self) -> Option<std::time::Duration> {
        if let (Some(mark_in), Some(mark_out), Some(duration)) =
            (self.mark_in, self.mark_out, self.duration)
        {
            let start_secs = duration.as_secs_f32() * mark_in;
            let end_secs = duration.as_secs_f32() * mark_out;
            let selection_secs = (end_secs - start_secs).abs();
            Some(std::time::Duration::from_secs_f32(selection_secs))
        } else {
            None
        }
    }

    pub fn open_save_dialog(&mut self) {
        if let Some(current_file) = &self.current_file {
            let path = std::path::Path::new(current_file);
            let parent = path.parent().unwrap_or(std::path::Path::new("."));

            // Generate suggested filename
            let base_name = path.file_stem().and_then(|s| s.to_str()).unwrap_or("audio");
            let source_extension = path.extension().and_then(|s| s.to_str()).unwrap_or("wav");

            // Always suggest WAV for selections (since we convert FLAC to WAV)
            // For full file saves, keep original extension
            let has_selection = self.mark_in.is_some() && self.mark_out.is_some();
            let extension = if has_selection {
                "wav" // Always WAV for selections
            } else {
                source_extension // Keep original for full file copies
            };

            let suggested_name = if has_selection {
                if self.edit_counter == 0 {
                    format!("{base_name}_edit.{extension}")
                } else {
                    format!("{}_edit_{}.{}", base_name, self.edit_counter + 1, extension)
                }
            } else {
                format!("{base_name}.{extension}")
            };

            self.save_dialog = Some(SaveDialog::new(
                parent.to_path_buf(),
                suggested_name,
                has_selection,
            ));

            info!(
                "Opened save dialog with filename: {}",
                self.save_dialog.as_ref().unwrap().filename
            );
        }
    }

    pub fn save_audio(
        &self,
        path: std::path::PathBuf,
        save_selection: bool,
    ) -> Result<(), Box<dyn Error>> {
        if let Some(current_file) = &self.current_file {
            if save_selection && self.mark_in.is_some() && self.mark_out.is_some() {
                // Save selection
                self.save_selection(current_file, path)
            } else {
                // Save full file (just copy)
                std::fs::copy(current_file, &path)?;
                info!("Copied full file to: {path:?}");
                Ok(())
            }
        } else {
            Err("No file loaded".into())
        }
    }

    fn save_selection(
        &self,
        source_path: &str,
        dest_path: std::path::PathBuf,
    ) -> Result<(), Box<dyn Error>> {
        let (mark_in, mark_out) = match (self.mark_in, self.mark_out) {
            (Some(a), Some(b)) => (a.min(b), a.max(b)),
            _ => return Err("No selection marks set".into()),
        };

        // Determine SOURCE format from extension
        let source_ext = std::path::Path::new(source_path)
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase())
            .unwrap_or_default();

        // Save the audio selection first
        let audio_result = match source_ext.as_str() {
            "wav" => self.save_wav_selection(source_path, dest_path.clone(), mark_in, mark_out),
            "flac" => {
                self.save_flac_to_wav_selection(source_path, dest_path.clone(), mark_in, mark_out)
            }
            _ => Err(format!("Unsupported source format: {source_ext}").into()),
        };

        // If audio save succeeded, try to clone and modify the sidecar file
        if audio_result.is_ok()
            && let Err(e) =
                self.clone_sidecar_for_selection(source_path, &dest_path, mark_in, mark_out)
        {
            log::warn!("Failed to create sidecar file: {e}");
            // Don't fail the entire operation if sidecar creation fails
        }

        audio_result
    }

    fn clone_sidecar_for_selection(
        &self,
        source_path: &str,
        dest_path: &std::path::Path,
        mark_in: f32,
        mark_out: f32,
    ) -> Result<(), Box<dyn Error>> {
        use std::fs;
        use std::path::PathBuf;

        // Construct source sidecar path (source_file.ext.md) safely
        let mut source_sidecar = PathBuf::from(source_path);
        let mut source_ext = source_sidecar
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("")
            .to_string();
        source_ext.push_str(".md");
        source_sidecar.set_extension(source_ext);

        // Construct destination sidecar path (dest_file.ext.md) safely
        let mut dest_sidecar = dest_path.to_path_buf();
        let mut dest_ext = dest_sidecar
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("")
            .to_string();
        dest_ext.push_str(".md");
        dest_sidecar.set_extension(dest_ext);

        // Calculate time ranges for documentation
        let duration_seconds = if let Some(duration) = self.duration {
            duration.as_secs_f32()
        } else {
            0.0
        };

        let start_time = (mark_in * duration_seconds) as u32;
        let end_time = (mark_out * duration_seconds) as u32;
        let selection_duration = end_time - start_time;

        let start_mins = start_time / 60;
        let start_secs = start_time % 60;
        let end_mins = end_time / 60;
        let end_secs = end_time % 60;
        let sel_mins = selection_duration / 60;
        let sel_secs = selection_duration % 60;
        let selection_duration_f32 = selection_duration as f32;

        // Simple ISO 8601 timestamp (approximate, good enough for this use case)
        let timestamp = {
            use std::process::Command;

            // Use system `date` command for proper ISO 8601 formatting
            if let Ok(output) = Command::new("date")
                .arg("-u")
                .arg("+%Y-%m-%dT%H:%M:%SZ")
                .output()
            {
                String::from_utf8_lossy(&output.stdout).trim().to_string()
            } else {
                // Fallback to basic format if date command fails
                let secs = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0);
                format!("unix-{secs}")
            }
        };

        let content = if source_sidecar.exists() {
            // Clone existing sidecar and add provenance fields to YAML frontmatter
            let original_content = fs::read_to_string(&source_sidecar)?;

            if let Some(frontmatter_end) = original_content.find("\n---\n") {
                // Has YAML frontmatter - update duration and insert our fields
                let yaml_section = &original_content[..frontmatter_end];
                let markdown_section = &original_content[frontmatter_end + 5..]; // Skip "\n---\n"

                // Update duration field if it exists, otherwise add it
                let updated_yaml = if let Some(duration_start) = yaml_section.find("duration:") {
                    // Find the end of the duration line
                    let after_duration = &yaml_section[duration_start..];
                    let duration_end = after_duration.find('\n').unwrap_or(after_duration.len());

                    // Replace the duration line
                    let before = &yaml_section[..duration_start];
                    let after = &yaml_section[duration_start + duration_end..];
                    format!("{before}duration: {selection_duration_f32:.2}{after}")
                } else {
                    // Add duration field
                    format!("{yaml_section}\nduration: {selection_duration_f32:.2}")
                };

                format!(
                    "{updated_yaml}\nsource_file: \"{source_path}\"\nsource_time_start: {start_mins}:{start_secs:02}\nsource_time_end: {end_mins}:{end_secs:02}\nsource_duration: {sel_mins}:{sel_secs:02}\nextracted_at: {timestamp}\nextraction_type: \"selection\"\n---\n{markdown_section}"
                )
            } else {
                // No YAML frontmatter - add it
                let dest_filename = dest_path
                    .file_name()
                    .and_then(|f| f.to_str())
                    .unwrap_or("Unknown");

                format!(
                    "---\ntitle: \"{dest_filename}\"\nduration: {selection_duration_f32:.2}\nsource_file: \"{source_path}\"\nsource_time_start: {start_mins}:{start_secs:02}\nsource_time_end: {end_mins}:{end_secs:02}\nsource_duration: {sel_mins}:{sel_secs:02}\nextracted_at: {timestamp}\nextraction_type: \"selection\"\n---\n\n{}",
                    original_content.trim()
                )
            }
        } else {
            // Create new sidecar with gentle reminder about missing source metadata
            let dest_filename = dest_path
                .file_name()
                .and_then(|f| f.to_str())
                .unwrap_or("Unknown");

            format!(
                r#"---
title: "{dest_filename}"
duration: {selection_duration_f32:.2}
tags: ["excerpt"]
source_file: "{source_path}"
source_time_start: {start_mins}:{start_secs:02}
source_time_end: {end_mins}:{end_secs:02}
source_duration: {sel_mins}:{sel_secs:02}
extracted_at: {timestamp}
extraction_type: "selection"
---

# {dest_filename}

**⚠️ Source file had no metadata**

This audio excerpt was extracted from a source file that did not have an associated sidecar (.md) file. Consider adding metadata to your source files for better organization and searchability.

## Notes

Add your notes and tags here to document this excerpt.
"#
            )
        };

        fs::write(&dest_sidecar, content)?;
        log::info!("Created sidecar file: {}", dest_sidecar.display());

        Ok(())
    }

    fn save_wav_selection(
        &self,
        source_path: &str,
        dest_path: std::path::PathBuf,
        start: f32,
        end: f32,
    ) -> Result<(), Box<dyn Error>> {
        use hound::{WavReader, WavWriter};
        use std::fs::File;
        use std::io::BufReader;

        // Open source file
        let reader = WavReader::new(BufReader::new(File::open(source_path)?))?;
        let spec = reader.spec();

        // Calculate sample range
        let (start_sample, samples_to_write) = self.calculate_sample_range(&reader, start, end);

        // Create output file
        let mut writer = WavWriter::create(&dest_path, spec)?;

        // Read and write samples based on bit depth
        self.copy_wav_samples(
            reader,
            &mut writer,
            spec.bits_per_sample,
            start_sample,
            samples_to_write,
        )?;

        writer.finalize()?;
        info!("Saved WAV selection to: {dest_path:?}");
        Ok(())
    }

    fn calculate_sample_range(
        &self,
        reader: &hound::WavReader<std::io::BufReader<std::fs::File>>,
        start: f32,
        end: f32,
    ) -> (usize, usize) {
        let total_samples = reader.len() as usize;
        let start_sample = (start * total_samples as f32) as usize;
        let end_sample = (end * total_samples as f32) as usize;
        let samples_to_write = end_sample - start_sample;
        (start_sample, samples_to_write)
    }

    fn copy_wav_samples<W: std::io::Write + std::io::Seek>(
        &self,
        mut reader: hound::WavReader<std::io::BufReader<std::fs::File>>,
        writer: &mut hound::WavWriter<W>,
        bits_per_sample: u16,
        start_sample: usize,
        samples_to_write: usize,
    ) -> Result<(), Box<dyn Error>> {
        match bits_per_sample {
            16 => self.copy_samples::<i16, _>(&mut reader, writer, start_sample, samples_to_write),
            24 | 32 => {
                self.copy_samples::<i32, _>(&mut reader, writer, start_sample, samples_to_write)
            }
            8 => self.copy_samples::<i8, _>(&mut reader, writer, start_sample, samples_to_write),
            _ => Err(format!("Unsupported bit depth: {bits_per_sample}").into()),
        }
    }

    fn copy_samples<T, W>(
        &self,
        reader: &mut hound::WavReader<std::io::BufReader<std::fs::File>>,
        writer: &mut hound::WavWriter<W>,
        start_sample: usize,
        samples_to_write: usize,
    ) -> Result<(), Box<dyn Error>>
    where
        T: hound::Sample + std::fmt::Debug,
        W: std::io::Write + std::io::Seek,
    {
        let samples: Vec<T> = reader
            .samples::<T>()
            .skip(start_sample)
            .take(samples_to_write)
            .collect::<Result<Vec<_>, _>>()?;

        for sample in samples {
            writer.write_sample(sample)?;
        }

        Ok(())
    }

    fn save_flac_to_wav_selection(
        &self,
        source_path: &str,
        dest_path: std::path::PathBuf,
        start: f32,
        end: f32,
    ) -> Result<(), Box<dyn Error>> {
        use claxon::FlacReader;
        use hound::{WavSpec, WavWriter};

        // Open FLAC file
        let reader = FlacReader::open(source_path)?;
        let info = reader.streaminfo();

        // Calculate sample range
        let total_samples = info.samples.unwrap_or(0) as usize;
        let start_sample = (start * total_samples as f32) as usize;
        let end_sample = (end * total_samples as f32) as usize;

        // Create WAV spec from FLAC info
        let spec = WavSpec {
            channels: info.channels as u16,
            sample_rate: info.sample_rate,
            bits_per_sample: 16, // Convert to 16-bit for compatibility
            sample_format: hound::SampleFormat::Int,
        };

        // Create output WAV file
        let mut writer = WavWriter::create(&dest_path, spec)?;

        // Read and convert samples
        self.convert_flac_samples(
            reader,
            &mut writer,
            info.bits_per_sample,
            start_sample,
            end_sample,
        )?;

        writer.finalize()?;
        info!("Saved FLAC selection as WAV to: {dest_path:?}");
        Ok(())
    }

    fn convert_flac_samples<W: std::io::Write + std::io::Seek>(
        &self,
        mut reader: claxon::FlacReader<std::fs::File>,
        writer: &mut hound::WavWriter<W>,
        bits_per_sample: u32,
        start_sample: usize,
        end_sample: usize,
    ) -> Result<(), Box<dyn Error>> {
        let mut sample_count = 0;

        for sample in reader.samples() {
            if sample_count >= start_sample && sample_count < end_sample {
                let sample = sample?;
                let sample_i16 = self.convert_sample_to_16bit(sample, bits_per_sample);
                writer.write_sample(sample_i16)?;
            }

            sample_count += 1;
            if sample_count >= end_sample {
                break;
            }
        }

        Ok(())
    }

    fn convert_sample_to_16bit(&self, sample: i32, bits_per_sample: u32) -> i16 {
        match bits_per_sample {
            16 => sample as i16,
            24 => (sample >> 8) as i16,
            32 => (sample >> 16) as i16,
            _ => (sample >> (bits_per_sample - 16)) as i16,
        }
    }

    pub fn open_sidecar_in_editor(&mut self) -> Result<Option<PathBuf>, Box<dyn Error>> {
        // Get the current file path
        let current_file = match &self.current_file {
            Some(file) => file,
            None => {
                self.editor_message = Some("No audio file loaded".to_string());
                self.editor_message_timer = Some(std::time::Instant::now());
                return Ok(None);
            }
        };

        // Construct sidecar path safely using set_extension
        let mut sidecar_path = PathBuf::from(current_file);
        let mut new_extension = sidecar_path
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("")
            .to_string();
        new_extension.push_str(".md");
        sidecar_path.set_extension(new_extension);

        // Check if sidecar exists
        if !sidecar_path.exists() {
            self.editor_message =
                Some("No sidecar file found - run 'zim update' first".to_string());
            self.editor_message_timer = Some(std::time::Instant::now());
            return Ok(None);
        }

        // Return the path to signal that we can open the editor
        Ok(Some(sidecar_path))
    }

    pub fn launch_editor(&mut self, sidecar_path: PathBuf) -> Result<(), Box<dyn Error>> {
        // Get editor from environment or use vim as fallback
        let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vim".to_string());

        // Basic security validation - reject if editor contains shell metacharacters
        // This helps prevent command injection attacks
        if editor.contains('|')
            || editor.contains(';')
            || editor.contains('&')
            || editor.contains('`')
            || editor.contains('$')
            || editor.contains('\n')
            || editor.contains('\r')
            || editor.contains('<')
            || editor.contains('>')
        {
            self.editor_message =
                Some("Invalid EDITOR value: contains shell metacharacters".to_string());
            self.editor_message_timer = Some(std::time::Instant::now());
            return Err("EDITOR environment variable contains invalid characters".into());
        }

        // Split editor command into program and args (handle cases like "vim -n")
        let editor_parts: Vec<&str> = editor.split_whitespace().collect();
        if editor_parts.is_empty() {
            return Err("EDITOR environment variable is empty".into());
        }

        let editor_cmd = editor_parts[0];
        let editor_args = &editor_parts[1..];

        // Launch editor in a subprocess
        let mut cmd = std::process::Command::new(editor_cmd);
        for arg in editor_args {
            cmd.arg(arg);
        }
        let status = cmd
            .arg(&sidecar_path)
            .stdin(std::process::Stdio::inherit())
            .stdout(std::process::Stdio::inherit())
            .stderr(std::process::Stdio::inherit())
            .status();

        match status {
            Ok(exit_status) => {
                if !exit_status.success() {
                    self.editor_message = Some(format!("Editor '{editor}' exited with error"));
                    self.editor_message_timer = Some(std::time::Instant::now());
                }
            }
            Err(e) => {
                self.editor_message = Some(format!("Failed to launch editor '{editor}': {e}"));
                self.editor_message_timer = Some(std::time::Instant::now());
            }
        }

        Ok(())
    }
}

pub fn run_with_file(
    file_path: Option<&str>,
    _gains: Option<Vec<f32>>,
) -> Result<(), Box<dyn Error>> {
    // Initialize logging
    init_logging()?;
    info!("Starting ZIM Audio Player");

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app and load file if provided
    let mut app = App::new();

    // Scan current directory for audio files
    info!("Scanning directory for audio files...");
    if let Err(e) = app.browser.scan_directory(std::path::Path::new(".")) {
        log::error!("Could not scan directory: {e}");
    }

    if let Some(path) = file_path
        && let Err(e) = app.load_file(path)
    {
        // Clean up terminal before showing error
        disable_raw_mode()?;
        execute!(
            terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture
        )?;
        terminal.show_cursor()?;
        return Err(e);
    }

    loop {
        let res = run_app(&mut terminal, &mut app);

        match res {
            Err(e) if e.to_string() == "EDITOR_REQUESTED" => {
                // Check if we can open the editor and get the sidecar path
                match app.open_sidecar_in_editor() {
                    Ok(Some(sidecar_path)) => {
                        // Store terminal state
                        let terminal_state_result = (|| -> Result<(), Box<dyn Error>> {
                            // Temporarily restore terminal for editor
                            disable_raw_mode()?;
                            execute!(
                                terminal.backend_mut(),
                                LeaveAlternateScreen,
                                DisableMouseCapture
                            )?;
                            terminal.show_cursor()?;
                            Ok(())
                        })();

                        // Only proceed if terminal state was successfully changed
                        if let Err(term_err) = terminal_state_result {
                            log::error!("Failed to prepare terminal for editor: {term_err}");
                            app.editor_message = Some(format!("Terminal error: {term_err}"));
                            app.editor_message_timer = Some(std::time::Instant::now());
                        } else {
                            // Launch editor
                            if let Err(editor_err) = app.launch_editor(sidecar_path) {
                                log::error!("Editor launch failed: {editor_err}");
                            }

                            // Always attempt to restore terminal state, even if editor failed
                            let restore_result = (|| -> Result<(), Box<dyn Error>> {
                                enable_raw_mode()?;
                                execute!(
                                    terminal.backend_mut(),
                                    EnterAlternateScreen,
                                    EnableMouseCapture
                                )?;
                                terminal.hide_cursor()?;
                                terminal.clear()?;
                                Ok(())
                            })();

                            if let Err(restore_err) = restore_result {
                                // Terminal restoration failed - this is critical
                                log::error!("Failed to restore terminal: {restore_err}");
                                return Err(
                                    format!("Terminal restoration failed: {restore_err}").into()
                                );
                            }
                        }
                    }
                    Ok(None) => {
                        // Editor cannot be opened (message already set in open_sidecar_in_editor)
                    }
                    Err(err) => {
                        log::error!("Error checking sidecar: {err}");
                        app.editor_message = Some(format!("Error: {err}"));
                        app.editor_message_timer = Some(std::time::Instant::now());
                    }
                }

                // Continue the main loop
                continue;
            }
            Err(e) => {
                // Restore terminal before showing error
                disable_raw_mode()?;
                execute!(
                    terminal.backend_mut(),
                    LeaveAlternateScreen,
                    DisableMouseCapture
                )?;
                terminal.show_cursor()?;

                eprintln!("Error: {e}");
                return Err(e);
            }
            Ok(_) => {
                // Normal exit
                break;
            }
        }
    }

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    Ok(())
}

pub fn run_with_files(
    file_paths: &[String],
    gains: Option<Vec<f32>>,
) -> Result<(), Box<dyn Error>> {
    // Initialize logging
    init_logging()?;
    info!("Starting ZIM Audio Player in mixing mode");

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app and load files for mixing
    let mut app = App::new();

    // Load multiple files
    if let Err(e) = app.load_files(file_paths, gains) {
        // Clean up terminal before showing error
        disable_raw_mode()?;
        execute!(
            terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture
        )?;
        terminal.show_cursor()?;
        return Err(e);
    }

    loop {
        let res = run_app(&mut terminal, &mut app);

        match res {
            Err(e) if e.to_string() == "EDITOR_REQUESTED" => {
                // Check if we can open the editor and get the sidecar path
                match app.open_sidecar_in_editor() {
                    Ok(Some(sidecar_path)) => {
                        // Store terminal state
                        let terminal_state_result = (|| -> Result<(), Box<dyn Error>> {
                            // Temporarily restore terminal for editor
                            disable_raw_mode()?;
                            execute!(
                                terminal.backend_mut(),
                                LeaveAlternateScreen,
                                DisableMouseCapture
                            )?;
                            terminal.show_cursor()?;
                            Ok(())
                        })();

                        // Only proceed if terminal state was successfully changed
                        if let Err(term_err) = terminal_state_result {
                            log::error!("Failed to prepare terminal for editor: {term_err}");
                            app.editor_message = Some(format!("Terminal error: {term_err}"));
                            app.editor_message_timer = Some(std::time::Instant::now());
                        } else {
                            // Launch editor
                            if let Err(editor_err) = app.launch_editor(sidecar_path) {
                                log::error!("Editor launch failed: {editor_err}");
                            }

                            // Always attempt to restore terminal state, even if editor failed
                            let restore_result = (|| -> Result<(), Box<dyn Error>> {
                                enable_raw_mode()?;
                                execute!(
                                    terminal.backend_mut(),
                                    EnterAlternateScreen,
                                    EnableMouseCapture
                                )?;
                                terminal.hide_cursor()?;
                                terminal.clear()?;
                                Ok(())
                            })();

                            if let Err(restore_err) = restore_result {
                                // Terminal restoration failed - this is critical
                                log::error!("Failed to restore terminal: {restore_err}");
                                return Err(
                                    format!("Terminal restoration failed: {restore_err}").into()
                                );
                            }
                        }
                    }
                    Ok(None) => {
                        // Editor cannot be opened (message already set in open_sidecar_in_editor)
                    }
                    Err(err) => {
                        log::error!("Error checking sidecar: {err}");
                        app.editor_message = Some(format!("Error: {err}"));
                        app.editor_message_timer = Some(std::time::Instant::now());
                    }
                }

                // Continue the main loop
                continue;
            }
            Err(e) => {
                // Restore terminal before showing error
                disable_raw_mode()?;
                execute!(
                    terminal.backend_mut(),
                    LeaveAlternateScreen,
                    DisableMouseCapture
                )?;
                terminal.show_cursor()?;

                eprintln!("Error: {e}");
                return Err(e);
            }
            Ok(_) => {
                // Normal exit
                break;
            }
        }
    }

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    Ok(())
}

fn run_app<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    app: &mut App,
) -> Result<(), Box<dyn Error>> {
    loop {
        // Update waveform data
        app.update_waveform();

        // Clear editor message after 3 seconds
        if let Some(timer) = app.editor_message_timer
            && timer.elapsed() > Duration::from_secs(3)
        {
            app.editor_message = None;
            // Use take() to properly clear the Option and avoid resource leak
            app.editor_message_timer.take();
        }

        terminal.draw(|f| ui::draw(f, app))?;

        // Poll for events with a short timeout to allow continuous rendering
        if event::poll(Duration::from_millis(50))?
            && let Event::Key(key) = event::read()?
        {
            handle_key_event(app, key)?;
        }

        if app.should_quit {
            return Ok(());
        }
    }
}

fn handle_key_event(app: &mut App, key: event::KeyEvent) -> Result<(), Box<dyn Error>> {
    if app.save_dialog.is_some() {
        handle_save_dialog_keys(app, key)
    } else {
        match app.view_mode {
            ViewMode::Player => handle_player_keys(app, key),
            ViewMode::Browser => handle_integrated_browser_keys(app, key),
        }
    }
}

fn handle_save_dialog_keys(app: &mut App, key: event::KeyEvent) -> Result<(), Box<dyn Error>> {
    use super::save_dialog::SaveDialogFocus;

    let save_dialog = app.save_dialog.as_mut().unwrap();

    match key.code {
        KeyCode::Esc => {
            app.save_dialog = None;
        }
        KeyCode::Tab => {
            save_dialog.toggle_focus();
        }
        KeyCode::Up => {
            if save_dialog.focus == SaveDialogFocus::DirectoryList {
                save_dialog.navigate_up();
            }
        }
        KeyCode::Down => {
            if save_dialog.focus == SaveDialogFocus::DirectoryList {
                save_dialog.navigate_down();
            }
        }
        KeyCode::Enter => {
            if save_dialog.focus == SaveDialogFocus::DirectoryList {
                save_dialog.enter_directory();
            } else {
                execute_save(app)?;
            }
        }
        KeyCode::Backspace => {
            save_dialog.pop_char();
        }
        KeyCode::Char(c) => {
            save_dialog.push_char(c);
        }
        _ => {}
    }
    Ok(())
}

fn execute_save(app: &mut App) -> Result<(), Box<dyn Error>> {
    if let Some(save_dialog) = &app.save_dialog {
        let save_path = save_dialog.get_full_path();
        let has_selection = save_dialog.has_selection;
        info!("Saving to: {save_path:?}");

        // Perform the save
        if let Err(e) = app.save_audio(save_path, has_selection) {
            log::error!("Failed to save audio: {e}");
            return Err(e);
        } else {
            app.edit_counter += 1;
        }
    }

    app.save_dialog = None;
    Ok(())
}

fn handle_integrated_browser_keys(
    app: &mut App,
    key: event::KeyEvent,
) -> Result<(), Box<dyn Error>> {
    use super::browser::BrowserFocus;

    // Universal keys that work regardless of focus
    match key.code {
        KeyCode::Esc => {
            // If search is visible, hide it first. Otherwise exit browser
            if app.browser.search_visible {
                app.browser.hide_search();
            } else {
                app.view_mode = ViewMode::Player;
            }
            return Ok(());
        }
        KeyCode::Tab => {
            // Only allow tab if search is visible
            if app.browser.search_visible {
                app.browser.toggle_focus();
            }
            return Ok(());
        }
        KeyCode::Left => {
            // Seek backward (works regardless of focus)
            if app.current_file.is_some() {
                if key.modifiers.contains(event::KeyModifiers::SHIFT) {
                    seek_audio_percentage(app, -0.2); // Jump back 20%
                } else {
                    seek_audio(app, -5.0); // Normal 5-second seek
                }
            }
            return Ok(());
        }
        KeyCode::Right => {
            // Seek forward (works regardless of focus)
            if app.current_file.is_some() {
                if key.modifiers.contains(event::KeyModifiers::SHIFT) {
                    seek_audio_percentage(app, 0.2); // Jump forward 20%
                } else {
                    seek_audio(app, 5.0); // Normal 5-second seek
                }
            }
            return Ok(());
        }
        _ => {}
    }

    // Focus-specific keys
    match app.browser.focus {
        BrowserFocus::Search => match key.code {
            KeyCode::Enter => {
                // Enter closes search and returns to file list
                app.browser.hide_search();
            }
            KeyCode::Backspace => app.browser.pop_char(),
            KeyCode::Char('c' | 'k') if key.modifiers.contains(event::KeyModifiers::CONTROL) => {
                app.browser.clear_search(); // Ctrl+C or Ctrl+K to clear search
            }
            KeyCode::Char(c) => app.browser.push_char(c),
            _ => {}
        },
        BrowserFocus::Files => {
            match key.code {
                KeyCode::Up | KeyCode::Char('k') => {
                    app.browser.select_previous();
                    // Auto-load the selected file for preview
                    preview_selected_file(app)?;
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    app.browser.select_next();
                    // Auto-load the selected file for preview
                    preview_selected_file(app)?;
                }
                KeyCode::Char('/') => {
                    // Show search box when / is pressed in file list
                    app.browser.show_search();
                }
                KeyCode::Char('h') => {
                    // Seek backward
                    if app.current_file.is_some() {
                        if key.modifiers.contains(event::KeyModifiers::SHIFT) {
                            seek_audio_percentage(app, -0.2); // Jump back 20%
                        } else {
                            seek_audio(app, -5.0); // Normal 5-second seek
                        }
                    }
                }
                KeyCode::Char('l') => {
                    // Seek forward
                    if app.current_file.is_some() {
                        if key.modifiers.contains(event::KeyModifiers::SHIFT) {
                            seek_audio_percentage(app, 0.2); // Jump forward 20%
                        } else {
                            seek_audio(app, 5.0); // Normal 5-second seek
                        }
                    }
                }
                KeyCode::Char(' ') => {
                    // Toggle play/pause (only in Files focus)
                    // First ensure we have the selected file loaded
                    if let Some(path) = app.browser.get_selected_path() {
                        let path_str = path.to_string_lossy().to_string();

                        // If no file is loaded or it's different from the selected one, load it
                        if app.current_file.as_ref() != Some(&path_str) {
                            preview_selected_file(app)?;
                        }

                        // Now toggle playback
                        app.toggle_playback();
                    }
                }
                KeyCode::Enter => {
                    // Load file and return to player
                    if let Some(path) = app.browser.get_selected_path() {
                        let path_str = path.to_string_lossy().to_string();

                        // Check if we're already playing this file
                        let was_playing_this_file = app.current_file.as_ref() == Some(&path_str);
                        let current_position = if was_playing_this_file {
                            Some(app.playback_position)
                        } else {
                            None
                        };
                        let was_playing = app.is_playing;

                        // Load the file (this resets position to 0)
                        app.load_file(&path_str)?;

                        // If we were previewing this file, restore the position
                        if let Some(position) = current_position {
                            // Seek to the previous position
                            if let Some(duration) = app.duration
                                && let Some(engine) = &mut app.audio_engine
                            {
                                // Since load_file resets to position 0, we need to seek forward
                                // by the absolute amount
                                let target_seconds = duration.as_secs_f32() * position;
                                engine.seek_relative(target_seconds)?;
                                // Note: playback_position will be updated by the next update_waveform call
                            }

                            // Restore play state if it was playing
                            if was_playing {
                                app.is_playing = true;
                                if let Some(engine) = &app.audio_engine {
                                    engine.play();
                                }
                            }
                        }

                        app.view_mode = ViewMode::Player;
                    }
                }
                _ => {}
            }
        }
    }

    Ok(())
}

fn preview_selected_file(app: &mut App) -> Result<(), Box<dyn Error>> {
    // Clone the path to avoid borrow issues
    let selected_path = app
        .browser
        .get_selected_path()
        .map(|p| p.to_string_lossy().to_string());

    if let Some(path_str) = selected_path {
        // Only load if it's an audio file and different from current
        if (path_str.ends_with(".wav")
            || path_str.ends_with(".flac")
            || path_str.ends_with(".aiff")
            || path_str.ends_with(".aif"))
            && app.current_file.as_ref() != Some(&path_str)
        {
            app.load_file(&path_str)?;
            // Don't auto-play, let user control with space
            app.is_playing = false;
            if let Some(engine) = &app.audio_engine {
                engine.pause();
            }
        }
    }
    Ok(())
}

fn handle_player_keys(app: &mut App, key: event::KeyEvent) -> Result<(), Box<dyn Error>> {
    match key.code {
        KeyCode::Char('q') => app.should_quit = true,
        KeyCode::Char(' ') => app.toggle_playback(),
        KeyCode::Char('b') => {
            app.view_mode = ViewMode::Browser;
            app.browser.hide_search(); // Ensure search is hidden when opening browser
            // Initialize browser with current directory
            app.browser.scan_directory(std::path::Path::new("."))?;
        }
        KeyCode::Char('/') => {
            app.view_mode = ViewMode::Browser;
            app.browser.hide_search(); // Start with search hidden, user can press '/' again to search
            // Initialize browser with current directory (preserves existing search)
            app.browser.scan_directory(std::path::Path::new("."))?;
        }
        KeyCode::Left => {
            if key.modifiers.contains(event::KeyModifiers::SHIFT) {
                seek_audio_percentage(app, -0.2); // Jump back 20%
            } else {
                seek_audio(app, -5.0); // Normal 5-second seek
            }
        }
        KeyCode::Right => {
            if key.modifiers.contains(event::KeyModifiers::SHIFT) {
                seek_audio_percentage(app, 0.2); // Jump forward 20%
            } else {
                seek_audio(app, 5.0); // Normal 5-second seek
            }
        }
        KeyCode::Char('[') | KeyCode::Char('i') => app.set_mark_in(),
        KeyCode::Char(']') | KeyCode::Char('o') => app.set_mark_out(),
        KeyCode::Char('x') => app.clear_marks(),
        KeyCode::Char('s') => app.open_save_dialog(),
        KeyCode::Char('l') => app.toggle_loop(),
        KeyCode::Char('e') => {
            // Signal that we want to open editor
            return Err("EDITOR_REQUESTED".into());
        }
        KeyCode::Char('t') => {
            if app.telemetry.config().enabled {
                app.disable_telemetry();
                info!("Audio telemetry disabled");
            } else {
                app.enable_debug_telemetry();
                info!("Audio telemetry enabled - press 't' again to disable");
            }
        }
        _ => {}
    }
    Ok(())
}

fn seek_audio(app: &mut App, seconds: f32) {
    if let Some(engine) = &mut app.audio_engine {
        let _ = engine.seek_relative(seconds);
    }
}

fn seek_audio_percentage(app: &mut App, percentage: f32) {
    if let (Some(engine), Some(duration)) = (&mut app.audio_engine, app.duration) {
        let seconds = duration.as_secs_f32() * percentage;
        let _ = engine.seek_relative(seconds);
    }
}

fn init_logging() -> Result<(), Box<dyn Error>> {
    use simplelog::*;
    use std::fs::File;

    let log_file = "/tmp/zim-player.log";
    CombinedLogger::init(vec![WriteLogger::new(
        LevelFilter::Debug,
        Config::default(),
        File::create(log_file)?,
    )])?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_new_app_initial_state() {
        let app = App::new();

        assert!(!app.should_quit);
        assert!(app.current_file.is_none());
        assert!(!app.is_playing);
        assert!(app.audio_engine.is_none());
        assert_eq!(app.left_level, 0.0);
        assert_eq!(app.right_level, 0.0);
        assert!(!app.is_stereo);
        assert_eq!(app.playback_position, 0.0);
        assert!(app.duration.is_none());
        assert!(app.mark_in.is_none());
        assert!(app.mark_out.is_none());
        assert!(app.save_dialog.is_none());
        assert!(!app.is_looping);
        assert_eq!(app.view_mode, ViewMode::Player);
    }

    #[test]
    fn test_set_mark_in() {
        let mut app = App::new();
        app.playback_position = 0.5;

        app.set_mark_in();

        assert_eq!(app.mark_in, Some(0.5));
    }

    #[test]
    fn test_set_mark_out() {
        let mut app = App::new();
        app.playback_position = 0.8;

        app.set_mark_out();

        assert_eq!(app.mark_out, Some(0.8));
    }

    #[test]
    fn test_clear_marks() {
        let mut app = App::new();
        app.mark_in = Some(0.2);
        app.mark_out = Some(0.8);
        app.is_looping = true;

        app.clear_marks();

        assert!(app.mark_in.is_none());
        assert!(app.mark_out.is_none());
        assert!(!app.is_looping);
    }

    #[test]
    fn test_get_selection_duration_no_marks() {
        let app = App::new();

        assert!(app.get_selection_duration().is_none());
    }

    #[test]
    fn test_get_selection_duration_with_marks() {
        let mut app = App::new();
        app.mark_in = Some(0.2);
        app.mark_out = Some(0.8);
        app.duration = Some(Duration::from_secs(10));

        let selection = app.get_selection_duration();

        assert!(selection.is_some());
        let duration = selection.unwrap();
        assert_eq!(duration.as_secs_f32(), 6.0); // (0.8 - 0.2) * 10
    }

    #[test]
    fn test_get_selection_duration_invalid_range() {
        let mut app = App::new();
        app.mark_in = Some(0.8);
        app.mark_out = Some(0.2); // Invalid: out before in
        app.duration = Some(Duration::from_secs(10));

        let selection = app.get_selection_duration();

        // The function uses abs() so it still returns a valid duration
        assert!(selection.is_some());
        let duration = selection.unwrap();
        assert_eq!(duration.as_secs_f32(), 6.0); // abs(0.2 - 0.8) * 10
    }

    #[test]
    fn test_toggle_playback_initial_state() {
        let mut app = App::new();

        // Without audio engine, toggle should not crash
        app.toggle_playback();

        assert!(!app.is_playing);
    }

    #[test]
    fn test_toggle_loop_without_marks() {
        let mut app = App::new();

        app.toggle_loop();

        // Should not enable looping without marks
        assert!(!app.is_looping);
    }

    #[test]
    fn test_toggle_loop_with_marks() {
        let mut app = App::new();
        app.mark_in = Some(0.2);
        app.mark_out = Some(0.8);

        app.toggle_loop();
        assert!(app.is_looping);

        app.toggle_loop();
        assert!(!app.is_looping);
    }

    #[test]
    fn test_open_save_dialog_without_file() {
        let mut app = App::new();

        app.open_save_dialog();

        // Dialog is only created when there's a current file
        assert!(app.save_dialog.is_none());
    }

    #[test]
    fn test_open_save_dialog_with_selection() {
        let mut app = App::new();
        app.current_file = Some("/path/to/audio.wav".to_string());
        app.mark_in = Some(0.2);
        app.mark_out = Some(0.8);
        app.edit_counter = 1;

        app.open_save_dialog();

        assert!(app.save_dialog.is_some());
        let dialog = app.save_dialog.as_ref().unwrap();
        assert!(dialog.has_selection);
        assert!(dialog.filename.contains("_edit"));
    }

    #[test]
    fn test_check_loop_boundaries_no_marks() {
        let app = App::new();
        assert!(app.check_loop_boundaries().is_none());
    }

    #[test]
    fn test_check_loop_boundaries_within_bounds() {
        let mut app = App::new();
        app.mark_in = Some(0.2);
        app.mark_out = Some(0.8);
        app.playback_position = 0.5;
        app.duration = Some(Duration::from_secs(10));

        assert!(app.check_loop_boundaries().is_none());
    }

    #[test]
    fn test_check_loop_boundaries_at_end() {
        let mut app = App::new();
        app.mark_in = Some(0.2);
        app.mark_out = Some(0.8);
        app.playback_position = 0.9; // Past the end
        app.duration = Some(Duration::from_secs(10));

        let seek = app.check_loop_boundaries();
        assert!(seek.is_some());
        assert!(seek.unwrap() < 0.0); // Should seek backwards
    }

    #[test]
    fn test_convert_sample_to_16bit() {
        let app = App::new();

        // Test 16-bit (no conversion)
        assert_eq!(app.convert_sample_to_16bit(1000, 16), 1000);

        // Test 24-bit conversion
        assert_eq!(app.convert_sample_to_16bit(256000, 24), 1000);

        // Test 32-bit conversion
        assert_eq!(app.convert_sample_to_16bit(65536000, 32), 1000);
    }

    #[test]
    fn test_apply_level_decay_playing() {
        let mut app = App::new();
        app.is_playing = true;
        app.left_level = 1.0;
        app.right_level = 1.0;

        app.apply_level_decay();

        assert!(app.left_level < 1.0);
        assert!(app.right_level < 1.0);
        assert_eq!(app.left_level, 0.99);
        assert_eq!(app.right_level, 0.99);
    }

    #[test]
    fn test_apply_level_decay_stopped() {
        let mut app = App::new();
        app.is_playing = false;
        app.left_level = 0.5;
        app.right_level = 0.5;

        app.apply_level_decay();

        assert_eq!(app.left_level, 0.0);
        assert_eq!(app.right_level, 0.0);
    }
}
