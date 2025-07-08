use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use log::info;
use ratatui::{Terminal, backend::CrosstermBackend};
use std::{error::Error, io, time::Duration};

use super::audio::AudioEngine;
use super::browser::Browser;
use super::save_dialog::SaveDialog;
use super::ui;
use super::waveform::WaveformBuffer;
use std::sync::mpsc;

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
        // Check for new audio samples
        if let Some(rx) = &self.samples_rx {
            while let Ok(samples) = rx.try_recv() {
                self.waveform_buffer.push_samples(&samples);

                // Calculate RMS levels for LED indicators
                if !samples.is_empty() {
                    // For stereo, assume interleaved samples (L, R, L, R...)
                    if self.is_stereo {
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
                        self.left_level =
                            ((left_sum / left_count.max(1) as f32).sqrt() * 2.0).min(1.0);
                        self.right_level =
                            ((right_sum / right_count.max(1) as f32).sqrt() * 2.0).min(1.0);
                    } else {
                        // Mono - same level for both
                        let sum: f32 = samples.iter().map(|s| s * s).sum();
                        // Mono - amplify for better visibility
                        let rms = ((sum / samples.len() as f32).sqrt() * 2.0).min(1.0);
                        self.left_level = rms;
                        self.right_level = rms;
                    }

                    // Apply gentler decay for better visibility
                    self.left_level = (self.left_level * 0.98).max(self.left_level * 0.8);
                    self.right_level = (self.right_level * 0.98).max(self.right_level * 0.8);
                }
            }
        }

        // Update playback position and handle looping
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
                if let (Some(mark_in), Some(mark_out)) = (self.mark_in, self.mark_out) {
                    let loop_start = mark_in.min(mark_out);
                    let loop_end = mark_in.max(mark_out);

                    // Check if we've reached the end of the loop
                    if self.playback_position >= loop_end {
                        // Calculate offset to jump back to start
                        if let Some(duration) = self.duration {
                            let current_seconds = duration.as_secs_f32() * self.playback_position;
                            let start_seconds = duration.as_secs_f32() * loop_start;
                            need_loop_seek = Some(start_seconds - current_seconds);
                        }
                    }
                    // Check if we're before the loop start (can happen after seeking)
                    else if self.playback_position < loop_start {
                        if let Some(duration) = self.duration {
                            let current_seconds = duration.as_secs_f32() * self.playback_position;
                            let start_seconds = duration.as_secs_f32() * loop_start;
                            need_loop_seek = Some(start_seconds - current_seconds);
                        }
                    }
                }
            }
        }
        
        // Stop playback if we've reached the end
        if should_stop {
            self.is_playing = false;
            if let Some(engine) = &self.audio_engine {
                engine.pause();
            }
        }

        // Apply loop seek if needed (now we can get mutable reference)
        if let (Some(offset), Some(engine)) = (need_loop_seek, &mut self.audio_engine) {
            let _ = engine.seek_relative(offset);
        }

        // Decay levels when not receiving samples
        if self.is_playing {
            self.left_level *= 0.99; // Slower decay for better visibility
            self.right_level *= 0.99;
        } else {
            self.left_level = 0.0;
            self.right_level = 0.0;
        }
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
            if self.is_looping {
                if let (Some(mark_in), Some(duration), Some(engine)) =
                    (self.mark_in, self.duration, &mut self.audio_engine)
                {
                    let start_seconds = duration.as_secs_f32() * mark_in;
                    let current_seconds = duration.as_secs_f32() * self.playback_position;
                    let offset = start_seconds - current_seconds;
                    let _ = engine.seek_relative(offset);
                }
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
                "wav"  // Always WAV for selections
            } else {
                source_extension  // Keep original for full file copies
            };

            let suggested_name = if has_selection {
                if self.edit_counter == 0 {
                    format!("{}_edit.{}", base_name, extension)
                } else {
                    format!("{}_edit_{}.{}", base_name, self.edit_counter + 1, extension)
                }
            } else {
                format!("{}.{}", base_name, extension)
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
    
    pub fn save_audio(&self, path: std::path::PathBuf, save_selection: bool) -> Result<(), Box<dyn Error>> {
        if let Some(current_file) = &self.current_file {
            if save_selection && self.mark_in.is_some() && self.mark_out.is_some() {
                // Save selection
                self.save_selection(current_file, path)
            } else {
                // Save full file (just copy)
                std::fs::copy(current_file, &path)?;
                info!("Copied full file to: {:?}", path);
                Ok(())
            }
        } else {
            Err("No file loaded".into())
        }
    }
    
    fn save_selection(&self, source_path: &str, dest_path: std::path::PathBuf) -> Result<(), Box<dyn Error>> {
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
        
        // Always save as WAV for now
        match source_ext.as_str() {
            "wav" => self.save_wav_selection(source_path, dest_path, mark_in, mark_out),
            "flac" => self.save_flac_to_wav_selection(source_path, dest_path, mark_in, mark_out),
            _ => Err(format!("Unsupported source format: {}", source_ext).into()),
        }
    }
    
    fn save_wav_selection(&self, source_path: &str, dest_path: std::path::PathBuf, start: f32, end: f32) -> Result<(), Box<dyn Error>> {
        use hound::{WavReader, WavWriter};
        use std::fs::File;
        use std::io::BufReader;
        
        // Open source file
        let mut reader = WavReader::new(BufReader::new(File::open(source_path)?))?;
        let spec = reader.spec();
        
        // Calculate sample range
        let total_samples = reader.len() as usize;
        let start_sample = (start * total_samples as f32) as usize;
        let end_sample = (end * total_samples as f32) as usize;
        let samples_to_write = end_sample - start_sample;
        
        // Create output file
        let mut writer = WavWriter::create(&dest_path, spec)?;
        
        // Read and write samples based on bit depth
        match spec.bits_per_sample {
            16 => {
                let samples: Vec<i16> = reader.samples::<i16>()
                    .skip(start_sample)
                    .take(samples_to_write)
                    .collect::<Result<Vec<_>, _>>()?;
                for sample in samples {
                    writer.write_sample(sample)?;
                }
            }
            24 | 32 => {
                let samples: Vec<i32> = reader.samples::<i32>()
                    .skip(start_sample)
                    .take(samples_to_write)
                    .collect::<Result<Vec<_>, _>>()?;
                for sample in samples {
                    writer.write_sample(sample)?;
                }
            }
            8 => {
                let samples: Vec<i8> = reader.samples::<i8>()
                    .skip(start_sample)
                    .take(samples_to_write)
                    .collect::<Result<Vec<_>, _>>()?;
                for sample in samples {
                    writer.write_sample(sample)?;
                }
            }
            _ => return Err(format!("Unsupported bit depth: {}", spec.bits_per_sample).into()),
        }
        
        writer.finalize()?;
        info!("Saved WAV selection to: {:?}", dest_path);
        Ok(())
    }
    
    fn save_flac_to_wav_selection(&self, source_path: &str, dest_path: std::path::PathBuf, start: f32, end: f32) -> Result<(), Box<dyn Error>> {
        use claxon::FlacReader;
        use hound::{WavWriter, WavSpec};
        
        // Open FLAC file
        let mut reader = FlacReader::open(source_path)?;
        let info = reader.streaminfo();
        
        // Calculate sample range
        let total_samples = info.samples.unwrap_or(0) as usize;
        let start_sample = (start * total_samples as f32) as usize;
        let end_sample = (end * total_samples as f32) as usize;
        let _samples_to_write = end_sample - start_sample;
        
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
        let mut sample_count = 0;
        for sample in reader.samples() {
            if sample_count >= start_sample && sample_count < end_sample {
                let sample = sample?;
                // Convert from FLAC bit depth to 16-bit
                let sample_i16 = match info.bits_per_sample {
                    16 => sample as i16,
                    24 => (sample >> 8) as i16,
                    32 => (sample >> 16) as i16,
                    _ => (sample >> (info.bits_per_sample - 16)) as i16,
                };
                writer.write_sample(sample_i16)?;
            }
            sample_count += 1;
            
            if sample_count >= end_sample {
                break;
            }
        }
        
        writer.finalize()?;
        info!("Saved FLAC selection as WAV to: {:?}", dest_path);
        Ok(())
    }
}

#[allow(dead_code)]
pub fn run() -> Result<(), Box<dyn Error>> {
    run_with_file(None)
}

pub fn run_with_file(file_path: Option<&str>) -> Result<(), Box<dyn Error>> {
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
        log::error!("Could not scan directory: {}", e);
    }

    if let Some(path) = file_path {
        if let Err(e) = app.load_file(path) {
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
    }

    let res = run_app(&mut terminal, app);

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        eprintln!("Error: {err}");
    }

    Ok(())
}

fn run_app<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    mut app: App,
) -> Result<(), Box<dyn Error>> {
    loop {
        // Update waveform data
        app.update_waveform();

        terminal.draw(|f| ui::draw(f, &app))?;

        // Poll for events with a short timeout to allow continuous rendering
        if event::poll(Duration::from_millis(50))? {
            if let Event::Key(key) = event::read()? {
                if let Some(ref mut save_dialog) = app.save_dialog {
                    // Handle save dialog navigation
                    use super::save_dialog::SaveDialogFocus;
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
                                // Save the file
                                let save_path = save_dialog.get_full_path();
                                let has_selection = save_dialog.has_selection;
                                info!("Saving to: {:?}", save_path);
                                
                                // Perform the save
                                if let Err(e) = app.save_audio(save_path, has_selection) {
                                    log::error!("Failed to save audio: {}", e);
                                } else {
                                    app.edit_counter += 1;
                                }
                                
                                app.save_dialog = None;
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
                } else if app.browser.is_active {
                    // Handle browser navigation
                    match key.code {
                        KeyCode::Esc => app.browser.toggle(),
                        KeyCode::Up => app.browser.select_previous(),
                        KeyCode::Down => app.browser.select_next(),
                        KeyCode::Enter => {
                            // Copy the path to avoid borrow issues
                            let selected_path = app
                                .browser
                                .get_selected_path()
                                .map(|p| p.to_string_lossy().to_string());

                            if let Some(path_str) = selected_path {
                                if let Err(e) = app.load_file(&path_str) {
                                    eprintln!("Error loading file: {}", e);
                                }
                                app.browser.toggle(); // Close browser after selection
                            }
                        }
                        KeyCode::Backspace => app.browser.pop_char(),
                        KeyCode::Char(c) => app.browser.push_char(c),
                        _ => {}
                    }
                } else {
                    // Normal player controls
                    match key.code {
                        KeyCode::Char('q') => app.should_quit = true,
                        KeyCode::Char(' ') => app.toggle_playback(),
                        KeyCode::Char('/') => app.browser.toggle(),
                        KeyCode::Left => {
                            if let Some(engine) = &mut app.audio_engine {
                                let _ = engine.seek_relative(-5.0); // Seek back 5 seconds
                            }
                        }
                        KeyCode::Right => {
                            if let Some(engine) = &mut app.audio_engine {
                                let _ = engine.seek_relative(5.0); // Seek forward 5 seconds
                            }
                        }
                        KeyCode::Char('[') | KeyCode::Char('i') => {
                            app.set_mark_in();
                        }
                        KeyCode::Char(']') | KeyCode::Char('o') => {
                            app.set_mark_out();
                        }
                        KeyCode::Char('x') => {
                            app.clear_marks();
                        }
                        KeyCode::Char('s') => {
                            app.open_save_dialog();
                        }
                        KeyCode::Char('l') => {
                            app.toggle_loop();
                        }
                        _ => {}
                    }
                }
            }
        }

        if app.should_quit {
            return Ok(());
        }
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
