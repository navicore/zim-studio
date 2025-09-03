//! Mixed audio source that combines multiple audio files with individual gain control.
//!
//! This module provides real-time mixing of up to 3 audio files, with per-file
//! gain control. All files are pre-mixed into memory for fast, high-quality seeking.

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
    create_mixed_source_from_files_with_seek(file_paths, gains, 0, samples_tx, samples_played)
}

/// Helper to create a mixed source from file paths with seek support
pub fn create_mixed_source_from_files_with_seek(
    file_paths: &[String],
    gains: Option<Vec<f32>>,
    start_sample: usize,
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

    log::info!(
        "Pre-mixing {} files into memory for fast seeking...",
        file_paths.len()
    );

    // Load all files into memory first
    let mut all_samples: Vec<Vec<f32>> = Vec::new();
    let mut sample_rate = 44100u32;
    let mut channels = 2u16;

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

        let (file_samples, file_sr, file_ch) = match ext.as_str() {
            "wav" => {
                let file = BufReader::new(File::open(path)?);
                let decoder = hound::WavReader::new(file)?;
                load_wav_samples(decoder)?
            }
            "flac" => {
                let reader = claxon::FlacReader::open(path)?;
                load_flac_samples(reader)?
            }
            "aif" | "aiff" => {
                let aiff_data = crate::media::metadata::read_aiff_data(path)?;
                load_aiff_samples(aiff_data)?
            }
            _ => return Err(format!("Unsupported audio format: {ext}").into()),
        };

        // Use properties from first file as reference
        if i == 0 {
            sample_rate = file_sr;
            channels = file_ch;
        } else {
            // Warn about mismatched properties
            if file_sr != sample_rate || file_ch != channels {
                log::warn!(
                    "File {path_str} properties mismatch: {file_sr}Hz/{file_ch}ch vs {sample_rate}Hz/{channels}ch"
                );
            }
        }

        all_samples.push(file_samples);
        log::info!("Loaded {} samples from {}", all_samples[i].len(), path_str);
    }

    // Pre-mix all samples into a single buffer
    let max_length = all_samples.iter().map(|s| s.len()).max().unwrap_or(0);
    let mut mixed_samples = vec![0.0f32; max_length];

    for (file_samples, gain) in all_samples.iter().zip(gains.iter()) {
        for (i, &sample) in file_samples.iter().enumerate() {
            if i < mixed_samples.len() {
                mixed_samples[i] += sample * gain;
            }
        }
    }

    // Clamp the mixed result to prevent clipping
    for sample in &mut mixed_samples {
        *sample = sample.clamp(-1.0, 1.0);
    }

    log::info!(
        "Pre-mixed {} samples at {}Hz/{}ch ({}MB in memory)",
        mixed_samples.len(),
        sample_rate,
        channels,
        (mixed_samples.len() * 4) / (1024 * 1024) // 4 bytes per f32
    );

    // Update the samples played counter to reflect the seek position
    samples_played.store(start_sample, Ordering::Relaxed);

    let pre_mixed_source = PreMixedSource::new(
        mixed_samples,
        sample_rate,
        channels,
        start_sample,
        samples_tx,
        samples_played,
    );

    Ok(Box::new(pre_mixed_source))
}

/// Load WAV file samples into memory
fn load_wav_samples(
    mut reader: hound::WavReader<std::io::BufReader<std::fs::File>>,
) -> Result<(Vec<f32>, u32, u16), Box<dyn std::error::Error>> {
    let spec = reader.spec();

    let samples = match spec.bits_per_sample {
        16 => {
            let samples: Result<Vec<i16>, _> = reader.samples().collect();
            samples?.into_iter().map(|s| s as f32 / 32768.0).collect()
        }
        24 => {
            let samples: Result<Vec<i32>, _> = reader.samples().collect();
            samples?.into_iter().map(|s| s as f32 / 8388608.0).collect()
        }
        32 => {
            let samples: Result<Vec<i32>, _> = reader.samples().collect();
            samples?
                .into_iter()
                .map(|s| s as f32 / 2147483648.0)
                .collect()
        }
        8 => {
            let samples: Result<Vec<i8>, _> = reader.samples().collect();
            samples?.into_iter().map(|s| (s as f32) / 128.0).collect()
        }
        _ => return Err(format!("Unsupported bit depth: {}", spec.bits_per_sample).into()),
    };

    Ok((samples, spec.sample_rate, spec.channels))
}

/// Load FLAC file samples into memory
fn load_flac_samples<R: std::io::Read>(
    mut reader: claxon::FlacReader<R>,
) -> Result<(Vec<f32>, u32, u16), Box<dyn std::error::Error>> {
    let info = reader.streaminfo();

    let mut samples = Vec::new();
    for sample in reader.samples() {
        let sample = sample?;
        let sample_f32 = match info.bits_per_sample {
            16 => sample as f32 / 32768.0,
            24 => sample as f32 / 8388608.0,
            _ => sample as f32 / 2147483648.0,
        };
        samples.push(sample_f32);
    }

    Ok((samples, info.sample_rate, info.channels as u16))
}

/// Load AIFF file samples into memory
fn load_aiff_samples(
    aiff_data: crate::media::metadata::AiffData,
) -> Result<(Vec<f32>, u32, u16), Box<dyn std::error::Error>> {
    let samples = aiff_data
        .audio_samples
        .into_iter()
        .map(|s| match aiff_data.bits_per_sample {
            16 => s as f32 / 32768.0,
            24 => s as f32 / 8388608.0,
            32 => s as f32 / 2147483648.0,
            8 => (s << 8) as f32 / 32768.0,
            _ => s as f32 / 32768.0,
        })
        .collect();

    Ok((samples, aiff_data.sample_rate, aiff_data.channels))
}

/// Pre-mixed source that holds all mixed audio in memory for fast seeking
struct PreMixedSource {
    mixed_samples: Vec<f32>,
    sample_rate: u32,
    channels: u16,
    position: usize,
    samples_tx: mpsc::Sender<Vec<f32>>,
    samples_played: Arc<AtomicUsize>,
    monitor_buffer: Vec<f32>,
}

impl PreMixedSource {
    fn new(
        mixed_samples: Vec<f32>,
        sample_rate: u32,
        channels: u16,
        start_position: usize,
        samples_tx: mpsc::Sender<Vec<f32>>,
        samples_played: Arc<AtomicUsize>,
    ) -> Self {
        let position = start_position.min(mixed_samples.len());
        Self {
            mixed_samples,
            sample_rate,
            channels,
            position,
            samples_tx,
            samples_played,
            monitor_buffer: Vec::with_capacity(2048),
        }
    }
}

impl Iterator for PreMixedSource {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        if self.position >= self.mixed_samples.len() {
            return None;
        }

        let sample = self.mixed_samples[self.position];
        self.position += 1;

        // Update samples played counter
        self.samples_played.fetch_add(1, Ordering::Relaxed);

        // Store sample for visualization
        self.monitor_buffer.push(sample);

        // Send visualization data in chunks (keeping stereo interleaving)
        let chunk_size = if self.channels > 1 { 2048 } else { 1024 };
        if self.monitor_buffer.len() >= chunk_size {
            let _ = self.samples_tx.send(self.monitor_buffer.clone());
            self.monitor_buffer.clear();
        }

        Some(sample)
    }
}

impl Source for PreMixedSource {
    fn current_span_len(&self) -> Option<usize> {
        Some(self.mixed_samples.len() - self.position)
    }

    fn channels(&self) -> u16 {
        self.channels
    }

    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    fn total_duration(&self) -> Option<Duration> {
        let total_samples = self.mixed_samples.len() as u64;
        let duration_secs = total_samples as f64 / (self.sample_rate as f64 * self.channels as f64);
        Some(Duration::from_secs_f64(duration_secs))
    }
}
