//! Full-timeline waveform for long audio file visualization.
//!
//! This module provides pre-calculated waveform data representing the entire
//! audio file, enabling fast navigation and visualization of long recordings.
//! Unlike the real-time oscilloscope buffer, this represents the complete timeline.

use hound::WavReader;
use std::error::Error;
use std::path::Path;

/// Downsampled waveform data representing the entire audio timeline
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct TimelineWaveform {
    /// Peak min/max pairs for each downsampled segment
    peaks: Vec<(f32, f32)>,
    /// Duration of the audio file in seconds
    duration: f32,
    /// Number of samples per peak pair (for reference)
    samples_per_peak: usize,
}

impl TimelineWaveform {
    /// Calculate waveform from a WAV file
    ///
    /// # Arguments
    /// * `path` - Path to the WAV file
    /// * `target_peaks` - Number of peak pairs to generate (typically 1000-2000 for display)
    ///
    /// # Returns
    /// A TimelineWaveform containing the downsampled peak data
    pub fn from_wav_file(path: &Path, target_peaks: usize) -> Result<Self, Box<dyn Error>> {
        let mut reader = WavReader::open(path)?;
        let spec = reader.spec();
        let sample_rate = spec.sample_rate as f32;
        let channels = spec.channels as usize;

        // Read all samples and convert to mono f32
        let samples: Vec<f32> = match spec.sample_format {
            hound::SampleFormat::Int => {
                let bits = spec.bits_per_sample;
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
        let duration = total_samples as f32 / sample_rate;
        let samples_per_peak = total_samples.div_ceil(target_peaks); // Round up

        // Calculate peaks
        let peaks: Vec<(f32, f32)> = mono_samples
            .chunks(samples_per_peak)
            .map(|chunk| {
                if chunk.is_empty() {
                    (0.0, 0.0)
                } else {
                    let min = chunk.iter().copied().fold(f32::INFINITY, f32::min);
                    let max = chunk.iter().copied().fold(f32::NEG_INFINITY, f32::max);
                    (min, max)
                }
            })
            .collect();

        Ok(Self {
            peaks,
            duration,
            samples_per_peak,
        })
    }

    /// Get the peak data for display
    #[allow(dead_code)]
    pub fn get_peaks(&self) -> &[(f32, f32)] {
        &self.peaks
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

    /// Get the duration of the audio in seconds
    #[allow(dead_code)]
    pub fn duration(&self) -> f32 {
        self.duration
    }

    /// Convert a time position (in seconds) to a peak index
    #[allow(dead_code)]
    pub fn time_to_peak_index(&self, time_seconds: f32) -> usize {
        let ratio = time_seconds / self.duration;
        let index = (ratio * self.peaks.len() as f32) as usize;
        index.min(self.peaks.len().saturating_sub(1))
    }

    /// Convert a peak index to a time position (in seconds)
    #[allow(dead_code)]
    pub fn peak_index_to_time(&self, index: usize) -> f32 {
        let ratio = index as f32 / self.peaks.len() as f32;
        ratio * self.duration
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
            duration: 3.0,
            samples_per_peak: 1000,
        };

        let display_peaks = waveform.get_display_peaks(3);
        assert_eq!(display_peaks.len(), 3);
        assert_eq!(display_peaks, peaks);
    }

    #[test]
    fn test_get_display_peaks_downsampled() {
        let peaks = vec![(-1.0, 1.0), (-0.8, 0.8), (-0.6, 0.6), (-0.4, 0.4)];
        let waveform = TimelineWaveform {
            peaks,
            duration: 4.0,
            samples_per_peak: 1000,
        };

        let display_peaks = waveform.get_display_peaks(2);
        assert_eq!(display_peaks.len(), 2);
        // First pixel should cover first two peaks: min=-1.0, max=1.0
        assert_eq!(display_peaks[0], (-1.0, 1.0));
        // Second pixel should cover last two peaks: min=-0.6, max=0.6
        assert_eq!(display_peaks[1], (-0.6, 0.6));
    }

    #[test]
    fn test_time_to_peak_index() {
        let waveform = TimelineWaveform {
            peaks: vec![(0.0, 0.0); 100],
            duration: 10.0,
            samples_per_peak: 1000,
        };

        assert_eq!(waveform.time_to_peak_index(0.0), 0);
        assert_eq!(waveform.time_to_peak_index(5.0), 50); // Middle
        assert_eq!(waveform.time_to_peak_index(10.0), 99); // End
    }

    #[test]
    fn test_peak_index_to_time() {
        let waveform = TimelineWaveform {
            peaks: vec![(0.0, 0.0); 100],
            duration: 10.0,
            samples_per_peak: 1000,
        };

        assert_eq!(waveform.peak_index_to_time(0), 0.0);
        assert!((waveform.peak_index_to_time(50) - 5.0).abs() < 0.01); // Middle
        assert!((waveform.peak_index_to_time(99) - 9.9).abs() < 0.01); // Near end
    }
}
