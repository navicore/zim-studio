//! Full-timeline waveform for long audio file visualization.
//!
//! This module provides pre-calculated waveform data representing the entire
//! audio file, enabling fast navigation and visualization of long recordings.
//! Unlike the real-time oscilloscope buffer, this represents the complete timeline.

use hound::WavReader;
use std::error::Error;
use std::path::Path;
use std::sync::mpsc::Sender;

/// Progress update for waveform calculation
#[derive(Debug, Clone)]
pub struct WaveformProgress {
    pub percentage: f32,
}

/// Downsampled waveform data representing the entire audio timeline
#[derive(Debug, Clone)]
pub struct TimelineWaveform {
    /// Peak min/max pairs for each downsampled segment
    peaks: Vec<(f32, f32)>,
}

impl TimelineWaveform {
    /// Calculate waveform from a WAV file with progress reporting
    ///
    /// This version sends progress updates through the provided channel,
    /// allowing for async calculation with UI feedback.
    pub fn from_wav_file_with_progress(
        path: &Path,
        target_peaks: usize,
        progress_tx: Option<Sender<WaveformProgress>>,
    ) -> Result<Self, Box<dyn Error>> {
        let mut reader = WavReader::open(path)?;
        let spec = reader.spec();
        let channels = spec.channels as usize;

        // Read all samples and convert to mono f32
        let samples: Vec<f32> = match spec.sample_format {
            hound::SampleFormat::Int => {
                let bits = spec.bits_per_sample;
                // Validate bit depth to prevent integer overflow in shift operation
                if !(1..=31).contains(&bits) {
                    return Err(format!("Unsupported bit depth: {bits} bits").into());
                }
                let max_value = (1i32 << (bits - 1)) as f32;
                reader
                    .samples::<i32>()
                    .filter_map(|s| s.ok())
                    .map(|s| s as f32 / max_value)
                    .collect()
            }
            hound::SampleFormat::Float => reader.samples::<f32>().filter_map(|s| s.ok()).collect(),
        };

        // Convert to mono if stereo
        let mono_samples: Vec<f32> = if channels == 2 {
            samples
                .chunks_exact(2)
                .map(|chunk| (chunk[0] + chunk[1]) / 2.0)
                .collect()
        } else {
            samples
        };

        let total_samples = mono_samples.len();
        let samples_per_peak = total_samples.div_ceil(target_peaks);

        // Calculate peaks with progress reporting
        let total_chunks = mono_samples.len().div_ceil(samples_per_peak);
        let peaks: Vec<(f32, f32)> = mono_samples
            .chunks(samples_per_peak)
            .enumerate()
            .map(|(idx, chunk)| {
                // Send progress update every 100 chunks
                if let Some(ref tx) = progress_tx
                    && idx % 100 == 0
                {
                    let _ = tx.send(WaveformProgress {
                        percentage: (idx as f32 / total_chunks as f32) * 100.0,
                    });
                }

                if chunk.is_empty() {
                    (0.0, 0.0)
                } else {
                    let min = chunk.iter().copied().fold(f32::INFINITY, f32::min);
                    let max = chunk.iter().copied().fold(f32::NEG_INFINITY, f32::max);
                    (min, max)
                }
            })
            .collect();

        // Send final progress update
        if let Some(ref tx) = progress_tx {
            let _ = tx.send(WaveformProgress { percentage: 100.0 });
        }

        Ok(Self { peaks })
    }

    /// Get a subset of peaks for the given display width
    ///
    /// If the display width is less than the number of peaks, this will
    /// downsample further by taking representative peaks.
    pub fn get_display_peaks(&self, display_width: usize) -> Vec<(f32, f32)> {
        if display_width >= self.peaks.len() {
            // No further downsampling needed
            return self.peaks.clone();
        }

        // Further downsample for display
        let peaks_per_pixel = self.peaks.len() as f32 / display_width as f32;

        (0..display_width)
            .map(|i| {
                let start_idx = (i as f32 * peaks_per_pixel) as usize;
                let end_idx = ((i + 1) as f32 * peaks_per_pixel) as usize;
                let end_idx = end_idx.min(self.peaks.len());

                if start_idx >= self.peaks.len() {
                    return (0.0, 0.0);
                }

                // Find min and max across this range of peaks
                let mut overall_min = f32::INFINITY;
                let mut overall_max = f32::NEG_INFINITY;

                for idx in start_idx..end_idx {
                    if let Some(&(min, max)) = self.peaks.get(idx) {
                        overall_min = overall_min.min(min);
                        overall_max = overall_max.max(max);
                    }
                }

                if overall_min.is_infinite() {
                    (0.0, 0.0)
                } else {
                    (overall_min, overall_max)
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_display_peaks_same_size() {
        let peaks = vec![(0.0, 1.0), (0.1, 0.9), (0.2, 0.8)];
        let waveform = TimelineWaveform {
            peaks: peaks.clone(),
        };

        let display_peaks = waveform.get_display_peaks(3);
        assert_eq!(display_peaks.len(), 3);
        assert_eq!(display_peaks, peaks);
    }

    #[test]
    fn test_get_display_peaks_downsampled() {
        let peaks = vec![(-1.0, 1.0), (-0.8, 0.8), (-0.6, 0.6), (-0.4, 0.4)];
        let waveform = TimelineWaveform { peaks };

        let display_peaks = waveform.get_display_peaks(2);
        assert_eq!(display_peaks.len(), 2);
        // First pixel should cover first two peaks: min=-1.0, max=1.0
        assert_eq!(display_peaks[0], (-1.0, 1.0));
        // Second pixel should cover last two peaks: min=-0.6, max=0.6
        assert_eq!(display_peaks[1], (-0.6, 0.6));
    }
}
