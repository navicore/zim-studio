//! Circular buffer for real-time waveform visualization.
//!
//! This module provides efficient storage for audio samples used in the oscilloscope
//! display. It maintains a fixed-size circular buffer that automatically discards
//! old samples as new ones arrive, providing a sliding window view of the audio
//! waveform suitable for real-time visualization.
//!
//! Also supports vectorscope visualization by storing stereo sample pairs.

use std::collections::VecDeque;

/// A stereo sample pair (left, right)
#[derive(Clone, Copy, Debug)]
pub struct StereoSample {
    pub left: f32,
    pub right: f32,
}

pub struct WaveformBuffer {
    samples: VecDeque<f32>,
    stereo_samples: VecDeque<StereoSample>, // For vectorscope
    max_samples: usize,
}

impl WaveformBuffer {
    pub fn new(max_samples: usize) -> Self {
        Self {
            samples: VecDeque::with_capacity(max_samples),
            stereo_samples: VecDeque::with_capacity(max_samples),
            max_samples,
        }
    }

    /// Push mono samples (or mixed-down stereo) for oscilloscope display
    pub fn push_samples(&mut self, new_samples: &[f32]) {
        for &sample in new_samples {
            self.samples.push_back(sample);
            // Keep only the most recent samples
            while self.samples.len() > self.max_samples {
                self.samples.pop_front();
            }
        }
    }

    /// Push interleaved stereo samples for both oscilloscope and vectorscope
    /// Samples should be in [L, R, L, R, ...] format
    pub fn push_stereo_samples(&mut self, new_samples: &[f32]) {
        // Push to mono buffer (mixed down for oscilloscope)
        for chunk in new_samples.chunks(2) {
            if chunk.len() == 2 {
                let mixed = (chunk[0] + chunk[1]) / 2.0;
                self.samples.push_back(mixed);
                while self.samples.len() > self.max_samples {
                    self.samples.pop_front();
                }

                // Also store as stereo pair for vectorscope
                self.stereo_samples.push_back(StereoSample {
                    left: chunk[0],
                    right: chunk[1],
                });
                while self.stereo_samples.len() > self.max_samples {
                    self.stereo_samples.pop_front();
                }
            }
        }
    }

    /// Get stereo sample pairs for vectorscope display
    /// Returns up to `count` most recent (left, right) pairs with amplification
    pub fn get_vectorscope_points(&self, count: usize) -> Vec<(f64, f64)> {
        // Amplification factor to spread out the display
        // Most recordings have moderate levels, so boost to fill the scope
        const VECTORSCOPE_GAIN: f64 = 2.5;

        if self.stereo_samples.is_empty() {
            return vec![];
        }

        // Downsample if we have more samples than requested
        let total = self.stereo_samples.len();
        let step = (total as f32 / count as f32).max(1.0);

        (0..count.min(total))
            .map(|i| {
                let idx = (i as f32 * step) as usize;
                if let Some(sample) = self.stereo_samples.get(idx) {
                    // Apply gain and clamp to display bounds
                    let left = (sample.left as f64 * VECTORSCOPE_GAIN).clamp(-0.98, 0.98);
                    let right = (sample.right as f64 * VECTORSCOPE_GAIN).clamp(-0.98, 0.98);
                    (left, right)
                } else {
                    (0.0, 0.0)
                }
            })
            .collect()
    }

    /// Get min/max pairs for peak-to-peak display without trigger stabilization.
    /// This provides better waveform visualization by showing the envelope.
    /// Use `get_triggered_display_peaks` for a stable oscilloscope view.
    #[allow(dead_code)]
    pub fn get_display_peaks(&self, count: usize) -> Vec<(f32, f32)> {
        self.get_peaks_from_offset(count, 0)
    }

    /// Get min/max pairs starting from a trigger point for stable oscilloscope display.
    /// Uses rising-edge zero-crossing detection to align the waveform consistently.
    pub fn get_triggered_display_peaks(&self, count: usize) -> Vec<(f32, f32)> {
        let trigger_offset = self.find_trigger_offset();
        self.get_peaks_from_offset(count, trigger_offset)
    }

    /// Find a rising-edge zero-crossing point to use as trigger.
    /// Searches the first portion of the buffer for a point where the signal
    /// crosses from negative to non-negative (rising edge at zero).
    fn find_trigger_offset(&self) -> usize {
        if self.samples.len() < 2 {
            return 0;
        }

        // Search the first quarter of the buffer for a trigger point
        // This leaves enough samples after the trigger for display
        let search_range = (self.samples.len() / 4).max(2);

        for i in 1..search_range {
            let prev = self.samples.get(i - 1).copied().unwrap_or(0.0);
            let curr = self.samples.get(i).copied().unwrap_or(0.0);

            // Rising edge: previous sample negative, current sample non-negative
            if prev < 0.0 && curr >= 0.0 {
                return i;
            }
        }

        // No trigger found, return 0 (start from beginning)
        0
    }

    /// Internal helper to get peaks from a given offset in the buffer
    fn get_peaks_from_offset(&self, count: usize, offset: usize) -> Vec<(f32, f32)> {
        if self.samples.is_empty() || offset >= self.samples.len() {
            return vec![(0.0, 0.0); count];
        }

        // Calculate how many samples we have after the offset
        let available_samples = self.samples.len() - offset;
        let samples_per_pixel = available_samples as f32 / count as f32;

        (0..count)
            .map(|i| {
                let start_idx = offset + (i as f32 * samples_per_pixel) as usize;
                let end_idx = offset + ((i + 1) as f32 * samples_per_pixel) as usize;
                let end_idx = end_idx.min(self.samples.len());

                if start_idx >= self.samples.len() {
                    return (0.0, 0.0);
                }

                // Find min and max in this window
                if start_idx == end_idx {
                    // Single sample
                    let sample = self.samples.get(start_idx).copied().unwrap_or(0.0);
                    (sample, sample)
                } else {
                    // Multiple samples - find peaks
                    let mut min = f32::INFINITY;
                    let mut max = f32::NEG_INFINITY;
                    for idx in start_idx..end_idx {
                        if let Some(&sample) = self.samples.get(idx) {
                            min = min.min(sample);
                            max = max.max(sample);
                        }
                    }
                    if min.is_infinite() {
                        (0.0, 0.0)
                    } else {
                        (min, max)
                    }
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_waveform_buffer_new() {
        let buffer = WaveformBuffer::new(100);
        assert_eq!(buffer.samples.len(), 0);
        assert_eq!(buffer.max_samples, 100);
    }

    #[test]
    fn test_push_samples() {
        let mut buffer = WaveformBuffer::new(5);
        buffer.push_samples(&[1.0, 2.0, 3.0]);
        assert_eq!(buffer.samples.len(), 3);

        buffer.push_samples(&[4.0, 5.0, 6.0]);
        assert_eq!(buffer.samples.len(), 5);

        // Should maintain max size
        let samples: Vec<f32> = buffer.samples.iter().copied().collect();
        assert_eq!(samples, vec![2.0, 3.0, 4.0, 5.0, 6.0]);
    }

    #[test]
    fn test_find_trigger_offset_rising_edge() {
        let mut buffer = WaveformBuffer::new(100);
        // Create a signal with enough samples so the search range covers the zero crossing
        // Search range = samples.len() / 4, so we need at least 16 samples to search up to index 4
        buffer.push_samples(&[
            -0.5, -0.3, -0.1, 0.1, 0.3, 0.5, 0.3, 0.1, -0.1, -0.3, -0.5, -0.3, -0.1, 0.1, 0.3, 0.5,
        ]);

        let offset = buffer.find_trigger_offset();
        // Should find the rising edge at index 3 (where -0.1 -> 0.1)
        assert_eq!(offset, 3);
    }

    #[test]
    fn test_find_trigger_offset_no_crossing() {
        let mut buffer = WaveformBuffer::new(100);
        // All positive samples - no zero crossing
        buffer.push_samples(&[0.1, 0.2, 0.3, 0.4, 0.5]);

        let offset = buffer.find_trigger_offset();
        // Should return 0 when no trigger found
        assert_eq!(offset, 0);
    }

    #[test]
    fn test_find_trigger_offset_empty() {
        let buffer = WaveformBuffer::new(100);
        let offset = buffer.find_trigger_offset();
        assert_eq!(offset, 0);
    }

    #[test]
    fn test_triggered_display_peaks() {
        let mut buffer = WaveformBuffer::new(100);
        // Create a signal with a clear zero crossing
        buffer.push_samples(&[
            -0.8, -0.6, -0.4, -0.2, 0.0, 0.2, 0.4, 0.6, 0.8, 1.0, 0.8, 0.6, 0.4, 0.2, 0.0, -0.2,
            -0.4, -0.6, -0.8, -1.0,
        ]);

        let peaks = buffer.get_triggered_display_peaks(5);
        // Should have 5 peak pairs
        assert_eq!(peaks.len(), 5);
        // First peak should start from the trigger point, not from -0.8
    }
}
