use rodio::{OutputStream, OutputStreamHandle, Sink, Source};
use std::error::Error;
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;
use std::sync::mpsc;
use std::time::Duration;

#[allow(dead_code)]
pub struct AudioInfo {
    pub channels: u16,
    pub sample_rate: u32,
}

pub struct AudioEngine {
    _stream: OutputStream,
    _stream_handle: OutputStreamHandle,
    sink: Sink,
    samples_tx: mpsc::Sender<Vec<f32>>,
    pub info: Option<AudioInfo>,
}

impl AudioEngine {
    pub fn new() -> Result<(Self, mpsc::Receiver<Vec<f32>>), Box<dyn Error>> {
        let (stream, stream_handle) = OutputStream::try_default()?;
        let sink = Sink::try_new(&stream_handle)?;
        let (samples_tx, samples_rx) = mpsc::channel();

        Ok((
            Self {
                _stream: stream,
                _stream_handle: stream_handle,
                sink,
                samples_tx,
                info: None,
            },
            samples_rx,
        ))
    }

    pub fn load_file(&mut self, path: &Path) -> Result<(), Box<dyn Error>> {
        // Stop any currently playing audio
        self.sink.stop();

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

        // Store audio info
        self.info = Some(AudioInfo {
            channels: spec.channels,
            sample_rate: spec.sample_rate,
        });

        // Create a monitoring source that sends samples to visualization
        let source = WavSource::new(reader, self.samples_tx.clone())?;

        // Play through rodio
        self.sink.append(source);

        println!(
            "Playing WAV: {} Hz, {} channels",
            spec.sample_rate, spec.channels
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
        let source = FlacSource::new(reader, self.samples_tx.clone())?;

        // Play through rodio
        self.sink.append(source);

        println!(
            "Playing FLAC: {} Hz, {} channels",
            info.sample_rate, info.channels
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
}

// Custom source that monitors samples for visualization
struct WavSource {
    samples_tx: mpsc::Sender<Vec<f32>>,
    sample_rate: u32,
    channels: u16,
    current_samples: Vec<i16>,
    position: usize,
    monitor_buffer: Vec<f32>,
}

impl WavSource {
    fn new(
        mut reader: hound::WavReader<BufReader<File>>,
        samples_tx: mpsc::Sender<Vec<f32>>,
    ) -> Result<Self, Box<dyn Error>> {
        let spec = reader.spec();

        // Read all samples upfront for simplicity
        let samples: Result<Vec<i16>, _> = reader.samples().collect();
        let samples = samples?;

        Ok(Self {
            samples_tx,
            sample_rate: spec.sample_rate,
            channels: spec.channels,
            current_samples: samples,
            position: 0,
            monitor_buffer: Vec::with_capacity(1024),
        })
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

        // Convert to f32 for visualization
        let normalized = sample as f32 / i16::MAX as f32;
        self.monitor_buffer.push(normalized);

        // Send visualization data in chunks (keeping stereo interleaving)
        // For stereo: buffer will contain L,R,L,R,L,R...
        let chunk_size = if self.channels > 1 { 2048 } else { 1024 };
        if self.monitor_buffer.len() >= chunk_size {
            let _ = self.samples_tx.send(self.monitor_buffer.clone());
            self.monitor_buffer.clear();
        }

        Some(sample)
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
}

impl FlacSource {
    fn new<R: Read>(
        mut reader: claxon::FlacReader<R>,
        samples_tx: mpsc::Sender<Vec<f32>>,
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
        })
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
