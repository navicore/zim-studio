//! Audio playback engine with real-time sample monitoring.
//!
//! This module provides the core audio functionality for the player, handling
//! file loading, playback control, and real-time audio sample streaming for
//! visualization. It supports multiple audio formats (WAV, FLAC) and provides
//! progress tracking and seeking capabilities.

use rodio::{OutputStream, OutputStreamBuilder, Sink, Source};
use std::error::Error;
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
    mpsc,
};
use std::time::Duration;

// Type alias for the audio engine creation result
type AudioEngineResult = Result<(AudioEngine, mpsc::Receiver<Vec<f32>>), Box<dyn Error>>;

pub struct AudioInfo {
    pub channels: u16,
    pub sample_rate: u32,
}

pub struct AudioEngine {
    _stream: OutputStream,
    sink: Sink,
    samples_tx: mpsc::Sender<Vec<f32>>,
    pub info: Option<AudioInfo>,
    pub duration: Option<Duration>,
    samples_played: Arc<AtomicUsize>,
    total_samples: usize,
    current_file_path: Option<String>,
    cached_aiff_data: Option<crate::media::metadata::AiffData>,
    // For mixed sources
    mixed_file_paths: Option<Vec<String>>,
    mixed_gains: Option<Vec<f32>>,
}

impl AudioEngine {
    pub fn new() -> AudioEngineResult {
        // Create output stream using rodio 0.21 API
        let stream = OutputStreamBuilder::open_default_stream()
            .map_err(|e| Box::new(e) as Box<dyn Error>)?;
        let sink = Sink::connect_new(stream.mixer());
        let (samples_tx, samples_rx) = mpsc::channel();

        Ok((
            Self {
                _stream: stream,
                sink,
                samples_tx,
                info: None,
                duration: None,
                samples_played: Arc::new(AtomicUsize::new(0)),
                total_samples: 0,
                current_file_path: None,
                cached_aiff_data: None,
                mixed_file_paths: None,
                mixed_gains: None,
            },
            samples_rx,
        ))
    }

    pub fn load_file(&mut self, path: &Path) -> Result<(), Box<dyn Error>> {
        // Stop any currently playing audio
        self.sink.stop();

        // Create a new sink for the new file
        // Note: We can't easily recreate the sink from stored stream in rodio 0.21
        // For now, we'll clear the current sink
        self.sink.stop();

        // Reset position tracking
        self.samples_played.store(0, Ordering::Relaxed);

        // Store the file path for seeking
        self.current_file_path = Some(path.to_string_lossy().to_string());

        // Open and decode the file
        let file = BufReader::new(File::open(path)?);

        // Try to decode based on extension
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase())
            .unwrap_or_default();

        match ext.as_str() {
            "wav" => {
                let decoder = hound::WavReader::new(file)?;
                self.play_wav(decoder)?;
            }
            "flac" => {
                self.play_flac(path)?;
            }
            "aif" | "aiff" => {
                self.play_aiff(path)?;
            }
            _ => return Err(format!("Unsupported audio format: {ext}").into()),
        }

        Ok(())
    }

    pub fn load_files(
        &mut self,
        paths: &[String],
        gains: Option<Vec<f32>>,
    ) -> Result<(), Box<dyn Error>> {
        // Stop any currently playing audio
        self.sink.stop();

        // Reset position tracking
        self.samples_played.store(0, Ordering::Relaxed);

        // Clear single file data since we're mixing multiple files
        self.current_file_path = None;
        self.cached_aiff_data = None;

        // Store mixed file information for seeking
        self.mixed_file_paths = Some(paths.to_vec());
        self.mixed_gains = gains.clone();

        // Create mixed source
        let mixed_source = crate::player::mixed_source::create_mixed_source_from_files(
            paths,
            gains,
            self.samples_tx.clone(),
            self.samples_played.clone(),
        )?;

        // Get info from mixed source
        self.info = Some(AudioInfo {
            channels: mixed_source.channels(),
            sample_rate: mixed_source.sample_rate(),
        });

        self.duration = mixed_source.total_duration();

        // For mixed sources, we can't easily calculate total samples
        // Use duration and sample rate to estimate
        if let Some(duration) = self.duration {
            let sample_rate = mixed_source.sample_rate() as f64;
            let channels = mixed_source.channels() as f64;
            self.total_samples = (duration.as_secs_f64() * sample_rate * channels) as usize;
        } else {
            self.total_samples = 0;
        }

        log::info!(
            "Playing mixed audio: {} files, {} Hz, {} channels",
            paths.len(),
            mixed_source.sample_rate(),
            mixed_source.channels()
        );

        // Play through rodio
        self.sink.append(mixed_source);

        Ok(())
    }

    fn load_files_from_position(
        &mut self,
        paths: &[String],
        gains: Option<Vec<f32>>,
        start_sample: usize,
    ) -> Result<(), Box<dyn Error>> {
        // Create mixed source with seeking support
        let mixed_source = crate::player::mixed_source::create_mixed_source_from_files_with_seek(
            paths,
            gains,
            start_sample,
            self.samples_tx.clone(),
            self.samples_played.clone(),
        )?;

        // Get info from mixed source
        self.info = Some(AudioInfo {
            channels: mixed_source.channels(),
            sample_rate: mixed_source.sample_rate(),
        });

        self.duration = mixed_source.total_duration();

        // For mixed sources, estimate total samples
        if let Some(duration) = self.duration {
            let sample_rate = mixed_source.sample_rate() as f64;
            let channels = mixed_source.channels() as f64;
            self.total_samples = (duration.as_secs_f64() * sample_rate * channels) as usize;
        } else {
            self.total_samples = 0;
        }

        log::info!(
            "Playing mixed audio from sample {}: {} files, {} Hz, {} channels",
            start_sample,
            paths.len(),
            mixed_source.sample_rate(),
            mixed_source.channels()
        );

        // Play through rodio
        self.sink.append(mixed_source);

        Ok(())
    }

    fn play_wav(
        &mut self,
        reader: hound::WavReader<BufReader<File>>,
    ) -> Result<(), Box<dyn Error>> {
        let spec = reader.spec();

        log::info!(
            "WAV format: {:?}, sample format: {:?}",
            spec,
            spec.sample_format
        );

        // Store audio info
        self.info = Some(AudioInfo {
            channels: spec.channels,
            sample_rate: spec.sample_rate,
        });

        // Create a monitoring source that sends samples to visualization
        let source = WavSource::new(reader, self.samples_tx.clone(), self.samples_played.clone())?;

        // Get duration from source
        self.duration = source.total_duration();
        self.total_samples = source.current_samples.len();

        log::info!(
            "WAV loaded: {} total samples, duration: {:?}",
            self.total_samples,
            self.duration
        );

        // Play through rodio
        self.sink.append(source);

        log::info!(
            "Playing WAV: {} Hz, {} channels, {} bits",
            spec.sample_rate,
            spec.channels,
            spec.bits_per_sample
        );

        Ok(())
    }

    fn play_flac(&mut self, path: &Path) -> Result<(), Box<dyn Error>> {
        let reader = claxon::FlacReader::open(path)?;
        let info = reader.streaminfo();

        // Store audio info
        self.info = Some(AudioInfo {
            channels: info.channels as u16,
            sample_rate: info.sample_rate,
        });

        // Create FLAC source
        let source = FlacSource::new(reader, self.samples_tx.clone(), self.samples_played.clone())?;

        // Get duration from source
        self.duration = source.total_duration();
        self.total_samples = source.current_samples.len();

        log::info!(
            "FLAC loaded: {} total samples, duration: {:?}",
            self.total_samples,
            self.duration
        );

        // Play through rodio
        self.sink.append(source);

        log::info!(
            "Playing FLAC: {} Hz, {} channels",
            info.sample_rate,
            info.channels
        );

        Ok(())
    }

    fn play_aiff(&mut self, path: &Path) -> Result<(), Box<dyn Error>> {
        // Load full AIFF file for best seek performance and seamless playback
        log::info!("Loading AIFF file: {}", path.display());
        let aiff_data = crate::media::metadata::read_aiff_data(path)?;

        log::info!(
            "AIFF loaded: {} total samples",
            aiff_data.audio_samples.len()
        );

        let source = AiffSource::from_data(
            aiff_data.clone(),
            self.samples_tx.clone(),
            self.samples_played.clone(),
        )?;

        // Cache the full data for fast seeking
        self.cached_aiff_data = Some(aiff_data.clone());

        self.info = Some(AudioInfo {
            sample_rate: source.sample_rate(),
            channels: source.channels(),
        });

        // Store the file path for seeking
        self.current_file_path = Some(path.to_string_lossy().to_string());

        // Set duration and total samples from our parser
        self.duration = source.total_duration();
        self.total_samples = source.total_samples();

        let calculated_duration = source.total_duration();
        log::info!(
            "AIFF loaded: {} Hz, {} channels, {} samples loaded (of {} total), calculated duration: {:?}",
            source.sample_rate(),
            source.channels(),
            aiff_data.audio_samples.len(),
            source.total_samples(),
            calculated_duration
        );

        // Log duration in seconds for easy comparison with QuickTime
        if let Some(duration) = calculated_duration {
            log::info!("AIFF duration: {:.2} seconds", duration.as_secs_f64());
        }

        // Play through rodio
        self.sink.append(source);

        Ok(())
    }

    pub fn play(&self) {
        self.sink.play();
    }

    pub fn pause(&self) {
        self.sink.pause();
    }

    pub fn get_progress(&self) -> f32 {
        // Return progress as 0.0 to 1.0
        if self.total_samples > 0 {
            let played = self.samples_played.load(Ordering::Relaxed);
            let progress = played as f32 / self.total_samples as f32;
            progress.min(1.0)
        } else {
            0.0
        }
    }

    fn load_file_from_position(
        &mut self,
        path: &Path,
        start_sample: usize,
    ) -> Result<(), Box<dyn Error>> {
        // Open and decode the file
        let file = BufReader::new(File::open(path)?);

        // Try to decode based on extension
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase())
            .unwrap_or_default();

        match ext.as_str() {
            "wav" => {
                let decoder = hound::WavReader::new(file)?;
                self.play_wav_from_position(decoder, start_sample)?;
            }
            "flac" => {
                self.play_flac_from_position(path, start_sample)?;
            }
            "aif" | "aiff" => {
                self.play_aiff_from_position(path, start_sample)?;
            }
            _ => return Err(format!("Unsupported audio format: {ext}").into()),
        }

        Ok(())
    }

    fn play_wav_from_position(
        &mut self,
        reader: hound::WavReader<BufReader<File>>,
        start_sample: usize,
    ) -> Result<(), Box<dyn Error>> {
        let _spec = reader.spec();

        // Create a monitoring source that sends samples to visualization
        let mut source =
            WavSource::new(reader, self.samples_tx.clone(), self.samples_played.clone())?;

        // Skip to the start position
        source.skip_to(start_sample);

        // Play through rodio
        self.sink.append(source);

        log::info!("Playing WAV from sample: {start_sample}");

        Ok(())
    }

    fn play_flac_from_position(
        &mut self,
        path: &Path,
        start_sample: usize,
    ) -> Result<(), Box<dyn Error>> {
        let reader = claxon::FlacReader::open(path)?;

        // Create FLAC source
        let mut source =
            FlacSource::new(reader, self.samples_tx.clone(), self.samples_played.clone())?;

        // Skip to the start position
        source.skip_to(start_sample);

        // Play through rodio
        self.sink.append(source);

        log::info!("Playing FLAC from sample: {start_sample}");

        Ok(())
    }

    fn play_aiff_from_position(
        &mut self,
        _path: &Path,
        start_sample: usize,
    ) -> Result<(), Box<dyn Error>> {
        // Use cached AIFF data for fast seeking
        if let Some(aiff_data) = &self.cached_aiff_data {
            let mut source = AiffSource::from_data(
                aiff_data.clone(),
                self.samples_tx.clone(),
                self.samples_played.clone(),
            )?;

            // Skip to the start position
            source.skip_to(start_sample);

            // Play through rodio
            self.sink.append(source);

            log::info!("Playing AIFF from sample: {start_sample}");
            Ok(())
        } else {
            Err("No cached AIFF data available for seeking".into())
        }
    }

    pub fn seek_relative(&mut self, seconds: f32) -> Result<(), Box<dyn Error>> {
        // Seek forward or backward by seconds
        if let Some(info) = &self.info {
            let samples_per_second = info.sample_rate as f32 * info.channels as f32;
            let sample_offset = (seconds * samples_per_second) as isize;

            let current = self.samples_played.load(Ordering::Relaxed) as isize;
            let new_position = (current + sample_offset).max(0) as usize;
            let new_position = new_position.min(self.total_samples);

            // Since rodio doesn't support seeking, we need to reload sources at the new position
            let was_playing = !self.sink.is_paused();

            // Stop current playback
            self.sink.stop();
            self.sink.stop(); // Double-stop for rodio 0.21 compatibility

            // Update position counter
            self.samples_played.store(new_position, Ordering::Relaxed);

            // Handle single file vs mixed files
            if let Some(path) = self.current_file_path.clone() {
                // Single file seeking
                self.load_file_from_position(Path::new(&path), new_position)?;
            } else if let (Some(paths), gains) =
                (self.mixed_file_paths.clone(), self.mixed_gains.clone())
            {
                // Mixed files seeking
                self.load_files_from_position(&paths, gains, new_position)?;
            }

            if was_playing {
                self.play();
            }

            log::info!(
                "Seek to sample {} ({}%)",
                new_position,
                (new_position as f32 / self.total_samples as f32 * 100.0) as u32
            );
        }
        Ok(())
    }
}

// Custom source that monitors samples for visualization
pub struct WavSource {
    samples_tx: mpsc::Sender<Vec<f32>>,
    sample_rate: u32,
    channels: u16,
    bits_per_sample: u16,
    current_samples: Vec<i32>, // Use i32 to handle up to 24-bit
    position: usize,
    monitor_buffer: Vec<f32>,
    samples_played: Arc<AtomicUsize>,
}

impl WavSource {
    pub fn new(
        mut reader: hound::WavReader<BufReader<File>>,
        samples_tx: mpsc::Sender<Vec<f32>>,
        samples_played: Arc<AtomicUsize>,
    ) -> Result<Self, Box<dyn Error>> {
        let spec = reader.spec();

        // Read samples based on bit depth
        let samples = match spec.bits_per_sample {
            16 => {
                let samples: Result<Vec<i16>, _> = reader.samples().collect();
                samples?.into_iter().map(|s| s as i32).collect()
            }
            24 => {
                let samples: Result<Vec<i32>, _> = reader.samples().collect();
                samples?
            }
            32 => {
                let samples: Result<Vec<i32>, _> = reader.samples().collect();
                samples?
            }
            8 => {
                let samples: Result<Vec<i8>, _> = reader.samples().collect();
                samples?.into_iter().map(|s| (s as i32) << 8).collect()
            }
            _ => return Err(format!("Unsupported bit depth: {}", spec.bits_per_sample).into()),
        };

        Ok(Self {
            samples_tx,
            sample_rate: spec.sample_rate,
            channels: spec.channels,
            bits_per_sample: spec.bits_per_sample,
            current_samples: samples,
            position: 0,
            monitor_buffer: Vec::with_capacity(1024),
            samples_played,
        })
    }

    fn skip_to(&mut self, sample_position: usize) {
        self.position = sample_position.min(self.current_samples.len());
    }
}

impl Iterator for WavSource {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        if self.position >= self.current_samples.len() {
            return None;
        }

        let sample = self.current_samples[self.position];
        self.position += 1;

        // Update samples played counter
        let _count = self.samples_played.fetch_add(1, Ordering::Relaxed);

        // Convert to f32 (rodio 0.21+ uses f32 samples)
        let sample_f32 = match self.bits_per_sample {
            16 => sample as f32 / 32768.0,       // i16 max
            24 => sample as f32 / 8388608.0,     // 24-bit max (2^23)
            32 => sample as f32 / 2147483648.0,  // i32 max
            8 => (sample << 8) as f32 / 32768.0, // Shift 8-bit and normalize
            _ => sample as f32 / 32768.0,
        };

        // Store normalized sample for visualization
        self.monitor_buffer.push(sample_f32);

        // Send visualization data in chunks (keeping stereo interleaving)
        // For stereo: buffer will contain L,R,L,R,L,R...
        let chunk_size = if self.channels > 1 { 2048 } else { 1024 };
        if self.monitor_buffer.len() >= chunk_size {
            let _ = self.samples_tx.send(self.monitor_buffer.clone());
            self.monitor_buffer.clear();
        }

        Some(sample_f32)
    }
}

impl Source for WavSource {
    fn current_span_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> u16 {
        self.channels
    }

    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    fn total_duration(&self) -> Option<Duration> {
        let total_samples = self.current_samples.len() as u64;
        let duration_secs = total_samples as f64 / (self.sample_rate as f64 * self.channels as f64);
        Some(Duration::from_secs_f64(duration_secs))
    }
}

// FLAC source with monitoring
pub struct FlacSource {
    samples_tx: mpsc::Sender<Vec<f32>>,
    sample_rate: u32,
    channels: u32,
    bits_per_sample: u32,
    current_samples: Vec<i32>,
    position: usize,
    monitor_buffer: Vec<f32>,
    samples_played: Arc<AtomicUsize>,
}

impl FlacSource {
    pub fn new<R: Read>(
        mut reader: claxon::FlacReader<R>,
        samples_tx: mpsc::Sender<Vec<f32>>,
        samples_played: Arc<AtomicUsize>,
    ) -> Result<Self, Box<dyn Error>> {
        let info = reader.streaminfo();

        // Read all samples
        let mut samples = Vec::new();
        for sample in reader.samples() {
            samples.push(sample?);
        }

        Ok(Self {
            samples_tx,
            sample_rate: info.sample_rate,
            channels: info.channels,
            bits_per_sample: info.bits_per_sample,
            current_samples: samples,
            position: 0,
            monitor_buffer: Vec::with_capacity(1024),
            samples_played,
        })
    }

    fn skip_to(&mut self, sample_position: usize) {
        self.position = sample_position.min(self.current_samples.len());
    }
}

impl Iterator for FlacSource {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        if self.position >= self.current_samples.len() {
            return None;
        }

        let sample = self.current_samples[self.position];
        self.position += 1;

        // Update samples played counter
        let _count = self.samples_played.fetch_add(1, Ordering::Relaxed);

        // Convert to f32 (rodio 0.21+ uses f32 samples)
        let sample_f32 = match self.bits_per_sample {
            16 => sample as f32 / 32768.0,     // i16 max
            24 => sample as f32 / 8388608.0,   // 24-bit max (2^23)
            _ => sample as f32 / 2147483648.0, // 32-bit max
        };

        // Store normalized sample for visualization
        self.monitor_buffer.push(sample_f32);

        // Send visualization data in chunks (keeping stereo interleaving)
        let chunk_size = if self.channels > 1 { 2048 } else { 1024 };
        if self.monitor_buffer.len() >= chunk_size {
            let _ = self.samples_tx.send(self.monitor_buffer.clone());
            self.monitor_buffer.clear();
        }

        Some(sample_f32)
    }
}

impl Source for FlacSource {
    fn current_span_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> u16 {
        self.channels as u16
    }

    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    fn total_duration(&self) -> Option<Duration> {
        let total_samples = self.current_samples.len() as u64;
        let duration_secs = total_samples as f64 / (self.sample_rate as f64 * self.channels as f64);
        Some(Duration::from_secs_f64(duration_secs))
    }
}

// AIFF source with monitoring
pub struct AiffSource {
    samples_tx: mpsc::Sender<Vec<f32>>,
    sample_rate: u32,
    channels: u16,
    bits_per_sample: u16,
    current_samples: Vec<i32>,
    position: usize,
    monitor_buffer: Vec<f32>,
    samples_played: Arc<AtomicUsize>,
}

impl AiffSource {
    pub fn from_data(
        aiff_data: crate::media::metadata::AiffData,
        samples_tx: mpsc::Sender<Vec<f32>>,
        samples_played: Arc<AtomicUsize>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        Ok(Self {
            samples_tx,
            sample_rate: aiff_data.sample_rate,
            channels: aiff_data.channels,
            bits_per_sample: aiff_data.bits_per_sample,
            current_samples: aiff_data.audio_samples,
            position: 0,
            monitor_buffer: Vec::new(),
            samples_played,
        })
    }

    fn skip_to(&mut self, sample_index: usize) {
        self.position = sample_index.min(self.current_samples.len());
    }

    fn total_samples(&self) -> usize {
        self.current_samples.len()
    }
}

impl Iterator for AiffSource {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        if self.position >= self.current_samples.len() {
            return None;
        }

        let sample = self.current_samples[self.position];
        self.position += 1;

        // Update samples played counter
        let _count = self.samples_played.fetch_add(1, Ordering::Relaxed);

        // Convert to f32 (rodio 0.21+ uses f32 samples)
        let sample_f32 = match self.bits_per_sample {
            16 => sample as f32 / 32768.0,       // i16 max
            24 => sample as f32 / 8388608.0,     // 24-bit max (2^23)
            32 => sample as f32 / 2147483648.0,  // i32 max
            8 => (sample << 8) as f32 / 32768.0, // Shift 8-bit and normalize
            _ => sample as f32 / 32768.0,
        };

        // Store normalized sample for visualization
        self.monitor_buffer.push(sample_f32);

        // Send samples in chunks of 1024 for visualization
        if self.monitor_buffer.len() >= 1024 {
            let _ = self.samples_tx.send(self.monitor_buffer.clone());
            self.monitor_buffer.clear();
        }

        Some(sample_f32)
    }
}

impl rodio::Source for AiffSource {
    fn current_span_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> u16 {
        self.channels
    }

    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    fn total_duration(&self) -> Option<Duration> {
        let total_samples = self.current_samples.len() as u64;
        let duration_secs = total_samples as f64 / (self.sample_rate as f64 * self.channels as f64);
        Some(Duration::from_secs_f64(duration_secs))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn is_ci_environment() -> bool {
        // Check common CI environment variables
        std::env::var("CI").is_ok()
            || std::env::var("GITHUB_ACTIONS").is_ok()
            || std::env::var("TRAVIS").is_ok()
            || std::env::var("CIRCLECI").is_ok()
            || std::env::var("GITLAB_CI").is_ok()
            || std::env::var("BUILDKITE").is_ok()
            || std::env::var("DRONE").is_ok()
    }

    fn skip_if_no_audio() -> Result<(), Box<dyn Error>> {
        if is_ci_environment() {
            eprintln!("CI environment detected - skipping audio test");
            eprintln!(
                "CI={:?}, GITHUB_ACTIONS={:?}",
                std::env::var("CI").ok(),
                std::env::var("GITHUB_ACTIONS").ok()
            );
            return Err("Audio not available in CI".into());
        }
        Ok(())
    }

    #[test]
    fn test_new_audio_engine() {
        if skip_if_no_audio().is_err() {
            return;
        }

        let result = AudioEngine::new();

        // In CI or systems without audio, this might fail
        if result.is_err() {
            eprintln!("Skipping test: AudioEngine creation failed (no audio device?)");
            return;
        }

        let (engine, rx) = result.unwrap();
        assert!(engine.info.is_none());
        assert!(engine.duration.is_none());
        assert_eq!(engine.total_samples, 0);
        assert!(engine.current_file_path.is_none());

        // Channel should be ready to receive
        assert!(rx.try_recv().is_err()); // Should be empty but not disconnected
    }

    #[test]
    fn test_audio_engine_initial_state() {
        if skip_if_no_audio().is_err() {
            return;
        }

        // Try to create engine, skip test if it fails (likely due to no audio device)
        let result = AudioEngine::new();
        if result.is_err() {
            eprintln!("Skipping test: AudioEngine creation failed (no audio device?)");
            return;
        }

        let (engine, _rx) = result.unwrap();

        // Check initial progress
        assert_eq!(engine.get_progress(), 0.0);
    }

    #[test]
    fn test_load_nonexistent_file() {
        if skip_if_no_audio().is_err() {
            return;
        }

        let result = AudioEngine::new();
        if result.is_err() {
            eprintln!("Skipping test: AudioEngine creation failed (no audio device?)");
            return;
        }

        let (mut engine, _rx) = result.unwrap();
        let result = engine.load_file(Path::new("/nonexistent/file.wav"));

        assert!(result.is_err());
    }

    #[test]
    fn test_load_unsupported_format() {
        if skip_if_no_audio().is_err() {
            return;
        }

        let result = AudioEngine::new();
        if result.is_err() {
            eprintln!("Skipping test: AudioEngine creation failed (no audio device?)");
            return;
        }

        let (mut engine, _rx) = result.unwrap();
        let result = engine.load_file(Path::new("test.mp3"));

        assert!(result.is_err());
        // The actual error depends on whether the file exists or not
        // If file doesn't exist, we get a file system error
        // So just check that it fails
    }

    #[test]
    fn test_play_pause_commands() {
        if skip_if_no_audio().is_err() {
            return;
        }

        let result = AudioEngine::new();
        if result.is_err() {
            eprintln!("Skipping test: AudioEngine creation failed (no audio device?)");
            return;
        }

        let (engine, _rx) = result.unwrap();

        // Test that play and pause commands don't panic
        engine.play();
        engine.pause();

        // Can't test actual state without a loaded file
    }

    #[test]
    fn test_seek_without_file() {
        if skip_if_no_audio().is_err() {
            return;
        }

        let result = AudioEngine::new();
        if result.is_err() {
            eprintln!("Skipping test: AudioEngine creation failed (no audio device?)");
            return;
        }

        let (mut engine, _rx) = result.unwrap();

        // Seeking without a loaded file should fail gracefully
        let result = engine.seek_relative(5.0);
        assert!(result.is_ok()); // Actually returns Ok(()) when no info
    }

    #[test]
    fn test_progress_without_file() {
        if skip_if_no_audio().is_err() {
            return;
        }

        let result = AudioEngine::new();
        if result.is_err() {
            eprintln!("Skipping test: AudioEngine creation failed (no audio device?)");
            return;
        }

        let (engine, _rx) = result.unwrap();

        // Progress without file should be 0
        assert_eq!(engine.get_progress(), 0.0);
    }
}
