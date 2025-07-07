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

            // Update channel info
            if let Some(info) = &engine.info {
                self.is_stereo = info.channels > 1;
            }

            self.current_file = Some(path.to_string());

            // Start playback automatically when file is loaded
            self.is_playing = true;
            engine.play();
        }

        Ok(())
    }

    pub fn toggle_playback(&mut self) {
        if let Some(engine) = &self.audio_engine {
            if self.is_playing {
                engine.pause();
                self.is_playing = false;
            } else {
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

        // Decay levels when not receiving samples
        if self.is_playing {
            self.left_level *= 0.99; // Slower decay for better visibility
            self.right_level *= 0.99;
        } else {
            self.left_level = 0.0;
            self.right_level = 0.0;
        }
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
                if app.browser.is_active {
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
