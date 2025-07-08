use rodio::{OutputStream, OutputStreamHandle, Sink, Source};
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

#[allow(dead_code)]
pub struct AudioInfo {
    pub channels: u16,
    pub sample_rate: u32,
}

pub struct AudioEngine {
    _stream: OutputStream,
    stream_handle: OutputStreamHandle,
    sink: Sink,
    samples_tx: mpsc::Sender<Vec<f32>>,
    pub info: Option<AudioInfo>,
    pub duration: Option<Duration>,
    samples_played: Arc<AtomicUsize>,
    total_samples: usize,
    current_file_path: Option<String>,
}

impl AudioEngine {
    pub fn new() -> Result<(Self, mpsc::Receiver<Vec<f32>>), Box<dyn Error>> {
        let (stream, stream_handle) = OutputStream::try_default()?;
        let sink = Sink::try_new(&stream_handle)?;
        let (samples_tx, samples_rx) = mpsc::channel();

        Ok((
            Self {
                _stream: stream,
                stream_handle,
                sink,
                samples_tx,
                info: None,
                duration: None,
                samples_played: Arc::new(AtomicUsize::new(0)),
                total_samples: 0,
                current_file_path: None,
            },
            samples_rx,
        ))
    }

    pub fn load_file(&mut self, path: &Path) -> Result<(), Box<dyn Error>> {
        // Stop any currently playing audio
        self.sink.stop();

        // Create a new sink for the new file
        self.sink = Sink::try_new(&self.stream_handle)?;

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
            _ => return Err(format!("Unsupported audio format: {}", ext).into()),
        }

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

    pub fn play(&self) {
        self.sink.play();
    }

    pub fn pause(&self) {
        self.sink.pause();
    }

    #[allow(dead_code)]
    pub fn is_paused(&self) -> bool {
        self.sink.is_paused()
    }

    #[allow(dead_code)]
    pub fn volume(&self) -> f32 {
        self.sink.volume()
    }

    #[allow(dead_code)]
    pub fn set_volume(&self, volume: f32) {
        self.sink.set_volume(volume);
    }

    pub fn get_progress(&self) -> f32 {
        // Return progress as 0.0 to 1.0
        if self.total_samples > 0 {
            let played = self.samples_played.load(Ordering::Relaxed);
            let progress = played as f32 / self.total_samples as f32;
            log::debug!(
                "Progress: {} / {} = {}",
                played,
                self.total_samples,
                progress
            );
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
            _ => return Err(format!("Unsupported audio format: {}", ext).into()),
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

        log::info!("Playing WAV from sample: {}", start_sample);

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

        log::info!("Playing FLAC from sample: {}", start_sample);

        Ok(())
    }

    pub fn seek_relative(&mut self, seconds: f32) -> Result<(), Box<dyn Error>> {
        // Seek forward or backward by seconds
        if let Some(info) = &self.info {
            let samples_per_second = info.sample_rate as f32 * info.channels as f32;
            let sample_offset = (seconds * samples_per_second) as isize;

            let current = self.samples_played.load(Ordering::Relaxed) as isize;
            let new_position = (current + sample_offset).max(0) as usize;
            let new_position = new_position.min(self.total_samples);

            // Since rodio doesn't support seeking, we need to reload the file at the new position
            if let Some(path) = self.current_file_path.clone() {
                let was_playing = !self.sink.is_paused();

                // Stop current playback
                self.sink.stop();

                // Create a new sink
                self.sink = Sink::try_new(&self.stream_handle)?;

                // Update position counter
                self.samples_played.store(new_position, Ordering::Relaxed);

                // Reload the file starting from the new position
                self.load_file_from_position(Path::new(&path), new_position)?;

                if was_playing {
                    self.play();
                }

                log::info!(
                    "Seek to sample {} ({}%)",
                    new_position,
                    (new_position as f32 / self.total_samples as f32 * 100.0) as u32
                );
            }
        }
        Ok(())
    }
}

// Custom source that monitors samples for visualization
struct WavSource {
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
    fn new(
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
    type Item = i16;

    fn next(&mut self) -> Option<Self::Item> {
        if self.position >= self.current_samples.len() {
            return None;
        }

        let sample = self.current_samples[self.position];
        self.position += 1;

        // Update samples played counter
        let count = self.samples_played.fetch_add(1, Ordering::Relaxed);
        if count % 44100 == 0 {
            // Log every second (assuming 44.1kHz)
            log::debug!("Samples played: {}", count);
        }

        // Convert to i16 based on bit depth
        let sample_i16 = match self.bits_per_sample {
            16 => sample as i16,
            24 => (sample >> 8) as i16,  // Shift 24-bit to 16-bit
            32 => (sample >> 16) as i16, // Shift 32-bit to 16-bit
            8 => (sample << 8) as i16,   // Shift 8-bit to 16-bit
            _ => sample as i16,
        };

        // Convert to f32 for visualization
        let normalized = sample as f32 / (1 << (self.bits_per_sample - 1)) as f32;
        self.monitor_buffer.push(normalized);

        // Send visualization data in chunks (keeping stereo interleaving)
        // For stereo: buffer will contain L,R,L,R,L,R...
        let chunk_size = if self.channels > 1 { 2048 } else { 1024 };
        if self.monitor_buffer.len() >= chunk_size {
            let _ = self.samples_tx.send(self.monitor_buffer.clone());
            self.monitor_buffer.clear();
        }

        Some(sample_i16)
    }
}

impl Source for WavSource {
    fn current_frame_len(&self) -> Option<usize> {
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
struct FlacSource {
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
    fn new<R: Read>(
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
    type Item = i16;

    fn next(&mut self) -> Option<Self::Item> {
        if self.position >= self.current_samples.len() {
            return None;
        }

        let sample = self.current_samples[self.position];
        self.position += 1;

        // Update samples played counter
        let count = self.samples_played.fetch_add(1, Ordering::Relaxed);
        if count % 44100 == 0 {
            // Log every second (assuming 44.1kHz)
            log::debug!("Samples played: {}", count);
        }

        // Convert to i16 based on bit depth
        let sample_i16 = match self.bits_per_sample {
            16 => sample as i16,
            24 => (sample >> 8) as i16,
            _ => (sample >> 16) as i16,
        };

        // Convert to f32 for visualization
        let normalized = sample as f32 / (1 << (self.bits_per_sample - 1)) as f32;
        self.monitor_buffer.push(normalized);

        // Send visualization data in chunks (keeping stereo interleaving)
        let chunk_size = if self.channels > 1 { 2048 } else { 1024 };
        if self.monitor_buffer.len() >= chunk_size {
            let _ = self.samples_tx.send(self.monitor_buffer.clone());
            self.monitor_buffer.clear();
        }

        Some(sample_i16)
    }
}

impl Source for FlacSource {
    fn current_frame_len(&self) -> Option<usize> {
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
