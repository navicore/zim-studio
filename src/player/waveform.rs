//! Circular buffer for real-time waveform visualization.
//!
//! This module provides efficient storage for audio samples used in the oscilloscope
//! display. It maintains a fixed-size circular buffer that automatically discards
//! old samples as new ones arrive, providing a sliding window view of the audio
//! waveform suitable for real-time visualization.

use std::collections::VecDeque;

pub struct WaveformBuffer {
    samples: VecDeque<f32>,
    max_samples: usize,
}

impl WaveformBuffer {
    pub fn new(max_samples: usize) -> Self {
        Self {
            samples: VecDeque::with_capacity(max_samples),
            max_samples,
        }
    }

    pub fn push_samples(&mut self, new_samples: &[f32]) {
        for &sample in new_samples {
            self.samples.push_back(sample);
            // Keep only the most recent samples
            while self.samples.len() > self.max_samples {
                self.samples.pop_front();
            }
        }
    }

    pub fn get_display_samples(&self, count: usize) -> Vec<f32> {
        if self.samples.is_empty() {
            return vec![0.0; count];
        }

        let step = self.samples.len() as f32 / count as f32;
        (0..count)
            .map(|i| {
                let idx = (i as f32 * step) as usize;
                self.samples.get(idx).copied().unwrap_or(0.0)
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
    fn test_get_display_samples_empty() {
        let buffer = WaveformBuffer::new(100);
        let samples = buffer.get_display_samples(10);
        assert_eq!(samples.len(), 10);
        assert!(samples.iter().all(|&s| s == 0.0));
    }

    #[test]
    fn test_get_display_samples_downsampling() {
        let mut buffer = WaveformBuffer::new(10);
        buffer.push_samples(&[1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0]);

        let samples = buffer.get_display_samples(5);
        assert_eq!(samples.len(), 5);
        // Should pick evenly spaced samples
        assert_eq!(samples[0], 1.0);
        assert_eq!(samples[2], 5.0);
        assert_eq!(samples[4], 9.0);
    }

    #[test]
    fn test_get_display_samples_upsampling() {
        let mut buffer = WaveformBuffer::new(5);
        buffer.push_samples(&[1.0, 2.0, 3.0]);

        let samples = buffer.get_display_samples(6);
        assert_eq!(samples.len(), 6);
        // Should repeat some samples when upsampling
    }
}
