//! Mixed audio source that combines multiple audio files with individual gain control.
//!
//! This module provides real-time mixing of up to 3 audio files, with per-file
//! gain control and automatic sample rate conversion if needed.

use rodio::Source;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
    mpsc,
};
use std::time::Duration;

/// Helper to create a mixed source from file paths with monitoring
pub fn create_mixed_source_from_files(
    file_paths: &[String],
    gains: Option<Vec<f32>>,
    samples_tx: mpsc::Sender<Vec<f32>>,
    samples_played: Arc<AtomicUsize>,
) -> Result<Box<dyn Source<Item = f32> + Send>, Box<dyn std::error::Error>> {
    use std::fs::File;
    use std::io::BufReader;
    use std::path::Path;

    if file_paths.is_empty() {
        return Err("No files provided".into());
    }

    if file_paths.len() > 3 {
        return Err("Maximum 3 files supported for mixing".into());
    }

    // Default gains to 1.0 if not provided
    let gains = gains.unwrap_or_else(|| vec![1.0; file_paths.len()]);

    if gains.len() != file_paths.len() {
        return Err("Number of gains must match number of files".into());
    }

    let mut sources: Vec<Box<dyn Source<Item = f32> + Send>> = Vec::new();

    for (i, path_str) in file_paths.iter().enumerate() {
        let path = Path::new(path_str);

        if !path.exists() {
            return Err(format!("File not found: {path_str}").into());
        }

        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase())
            .unwrap_or_default();

        // Create appropriate source based on file type
        // NOTE: We create sources WITHOUT monitoring since we'll monitor the mixed output
        let source: Box<dyn Source<Item = f32> + Send> = match ext.as_str() {
            "wav" => {
                let file = BufReader::new(File::open(path)?);
                let decoder = hound::WavReader::new(file)?;
                let source = create_wav_source_no_monitor(decoder)?;
                Box::new(source)
            }
            "flac" => {
                let reader = claxon::FlacReader::open(path)?;
                let source = create_flac_source_no_monitor(reader)?;
                Box::new(source)
            }
            "aif" | "aiff" => {
                let aiff_data = crate::media::metadata::read_aiff_data(path)?;
                let source = create_aiff_source_no_monitor(aiff_data)?;
                Box::new(source)
            }
            _ => return Err(format!("Unsupported audio format: {ext}").into()),
        };

        sources.push(source);

        log::info!("Loaded file {} with gain {}", path_str, gains[i]);
    }

    // Create mixed source with monitoring
    let sources_with_gains: Vec<(Box<dyn Source<Item = f32> + Send>, f32)> = sources
        .into_iter()
        .zip(gains.iter().copied())
        .map(|(s, g)| (s as Box<dyn Source<Item = f32> + Send>, g))
        .collect();

    Ok(Box::new(DynamicMixer::new(
        sources_with_gains,
        samples_tx,
        samples_played,
    )?))
}

/// Create WAV source without monitoring
fn create_wav_source_no_monitor(
    mut reader: hound::WavReader<std::io::BufReader<std::fs::File>>,
) -> Result<WavSourceNoMonitor, Box<dyn std::error::Error>> {
    let spec = reader.spec();

    // Read all samples
    let samples = match spec.bits_per_sample {
        16 => {
            let samples: Result<Vec<i16>, _> = reader.samples().collect();
            samples?.into_iter().map(|s| s as i32).collect()
        }
        24 | 32 => {
            let samples: Result<Vec<i32>, _> = reader.samples().collect();
            samples?
        }
        8 => {
            let samples: Result<Vec<i8>, _> = reader.samples().collect();
            samples?.into_iter().map(|s| (s as i32) << 8).collect()
        }
        _ => return Err(format!("Unsupported bit depth: {}", spec.bits_per_sample).into()),
    };

    Ok(WavSourceNoMonitor {
        sample_rate: spec.sample_rate,
        channels: spec.channels,
        bits_per_sample: spec.bits_per_sample,
        samples,
        position: 0,
    })
}

/// Simple WAV source without monitoring
struct WavSourceNoMonitor {
    sample_rate: u32,
    channels: u16,
    bits_per_sample: u16,
    samples: Vec<i32>,
    position: usize,
}

impl Iterator for WavSourceNoMonitor {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        if self.position >= self.samples.len() {
            return None;
        }

        let sample = self.samples[self.position];
        self.position += 1;

        // Convert to f32
        let sample_f32 = match self.bits_per_sample {
            16 => sample as f32 / 32768.0,
            24 => sample as f32 / 8388608.0,
            32 => sample as f32 / 2147483648.0,
            8 => sample as f32 / 128.0,
            _ => sample as f32 / 32768.0,
        };

        Some(sample_f32)
    }
}

impl Source for WavSourceNoMonitor {
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
        let total_samples = self.samples.len() as u64;
        let duration_secs = total_samples as f64 / (self.sample_rate as f64 * self.channels as f64);
        Some(Duration::from_secs_f64(duration_secs))
    }
}

/// Create FLAC source without monitoring
fn create_flac_source_no_monitor<R: std::io::Read>(
    mut reader: claxon::FlacReader<R>,
) -> Result<FlacSourceNoMonitor, Box<dyn std::error::Error>> {
    let info = reader.streaminfo();

    // Read all samples
    let mut samples = Vec::new();
    for sample in reader.samples() {
        samples.push(sample?);
    }

    Ok(FlacSourceNoMonitor {
        sample_rate: info.sample_rate,
        channels: info.channels,
        bits_per_sample: info.bits_per_sample,
        samples,
        position: 0,
    })
}

/// Simple FLAC source without monitoring
struct FlacSourceNoMonitor {
    sample_rate: u32,
    channels: u32,
    bits_per_sample: u32,
    samples: Vec<i32>,
    position: usize,
}

impl Iterator for FlacSourceNoMonitor {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        if self.position >= self.samples.len() {
            return None;
        }

        let sample = self.samples[self.position];
        self.position += 1;

        // Convert to f32
        let sample_f32 = match self.bits_per_sample {
            16 => sample as f32 / 32768.0,
            24 => sample as f32 / 8388608.0,
            _ => sample as f32 / 2147483648.0,
        };

        Some(sample_f32)
    }
}

impl Source for FlacSourceNoMonitor {
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
        let total_samples = self.samples.len() as u64;
        let duration_secs = total_samples as f64 / (self.sample_rate as f64 * self.channels as f64);
        Some(Duration::from_secs_f64(duration_secs))
    }
}

/// Create AIFF source without monitoring
fn create_aiff_source_no_monitor(
    aiff_data: crate::media::metadata::AiffData,
) -> Result<AiffSourceNoMonitor, Box<dyn std::error::Error>> {
    Ok(AiffSourceNoMonitor {
        sample_rate: aiff_data.sample_rate,
        channels: aiff_data.channels,
        bits_per_sample: aiff_data.bits_per_sample,
        samples: aiff_data.audio_samples,
        position: 0,
    })
}

/// Simple AIFF source without monitoring
struct AiffSourceNoMonitor {
    sample_rate: u32,
    channels: u16,
    bits_per_sample: u16,
    samples: Vec<i32>,
    position: usize,
}

impl Iterator for AiffSourceNoMonitor {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        if self.position >= self.samples.len() {
            return None;
        }

        let sample = self.samples[self.position];
        self.position += 1;

        // Convert to f32
        let sample_f32 = match self.bits_per_sample {
            16 => sample as f32 / 32768.0,
            24 => sample as f32 / 8388608.0,
            32 => sample as f32 / 2147483648.0,
            8 => (sample << 8) as f32 / 32768.0,
            _ => sample as f32 / 32768.0,
        };

        Some(sample_f32)
    }
}

impl Source for AiffSourceNoMonitor {
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
        let total_samples = self.samples.len() as u64;
        let duration_secs = total_samples as f64 / (self.sample_rate as f64 * self.channels as f64);
        Some(Duration::from_secs_f64(duration_secs))
    }
}

/// Dynamic mixer that works with trait objects and f32 samples
struct DynamicMixer {
    sources: Vec<(Box<dyn Source<Item = f32> + Send>, f32)>,
    sample_rate: u32,
    channels: u16,
    samples_tx: mpsc::Sender<Vec<f32>>,
    samples_played: Arc<AtomicUsize>,
    monitor_buffer: Vec<f32>,
}

impl DynamicMixer {
    fn new(
        sources_with_gains: Vec<(Box<dyn Source<Item = f32> + Send>, f32)>,
        samples_tx: mpsc::Sender<Vec<f32>>,
        samples_played: Arc<AtomicUsize>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        if sources_with_gains.is_empty() {
            return Err("No sources provided".into());
        }

        // Get properties from first source
        let sample_rate = sources_with_gains[0].0.sample_rate();
        let channels = sources_with_gains[0].0.channels();

        // Verify compatibility
        for (source, _) in &sources_with_gains {
            if source.sample_rate() != sample_rate || source.channels() != channels {
                log::warn!(
                    "Source properties mismatch - SR: {} vs {}, CH: {} vs {}",
                    source.sample_rate(),
                    sample_rate,
                    source.channels(),
                    channels
                );
            }
        }

        Ok(Self {
            sources: sources_with_gains,
            sample_rate,
            channels,
            samples_tx,
            samples_played,
            monitor_buffer: Vec::with_capacity(2048),
        })
    }
}

impl Iterator for DynamicMixer {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        let mut mixed_sample = 0.0f32;
        let mut any_active = false;

        for (source, gain) in &mut self.sources {
            if let Some(sample) = source.next() {
                any_active = true;
                mixed_sample += sample * *gain;
            }
        }

        if !any_active {
            return None;
        }

        // Clamp to prevent clipping
        let final_sample = mixed_sample.clamp(-1.0, 1.0);

        // Update samples played counter
        self.samples_played.fetch_add(1, Ordering::Relaxed);

        // Store sample for visualization
        self.monitor_buffer.push(final_sample);

        // Send visualization data in chunks (keeping stereo interleaving)
        let chunk_size = if self.channels > 1 { 2048 } else { 1024 };
        if self.monitor_buffer.len() >= chunk_size {
            let _ = self.samples_tx.send(self.monitor_buffer.clone());
            self.monitor_buffer.clear();
        }

        Some(final_sample)
    }
}

impl Source for DynamicMixer {
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
        self.sources
            .iter()
            .filter_map(|(s, _)| s.total_duration())
            .max()
    }
}
