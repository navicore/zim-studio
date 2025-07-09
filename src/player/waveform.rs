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

    #[allow(dead_code)]
    pub fn clear(&mut self) {
        self.samples.clear();
    }
}

/// Convert amplitude to terminal block characters for visualization
#[allow(dead_code)]
pub fn amplitude_to_blocks(amplitude: f32) -> &'static str {
    let normalized = amplitude.abs().min(1.0);
    let index = (normalized * 8.0) as usize;

    match index {
        0 => " ",
        1 => "▁",
        2 => "▂",
        3 => "▃",
        4 => "▄",
        5 => "▅",
        6 => "▆",
        7 => "▇",
        _ => "█",
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

    #[test]
    fn test_clear() {
        let mut buffer = WaveformBuffer::new(10);
        buffer.push_samples(&[1.0, 2.0, 3.0]);
        assert_eq!(buffer.samples.len(), 3);

        buffer.clear();
        assert_eq!(buffer.samples.len(), 0);
    }

    #[test]
    fn test_amplitude_to_blocks() {
        assert_eq!(amplitude_to_blocks(0.0), " ");
        assert_eq!(amplitude_to_blocks(0.12), " "); // 0.12 * 8 = 0.96 -> 0
        assert_eq!(amplitude_to_blocks(0.13), "▁"); // 0.13 * 8 = 1.04 -> 1
        assert_eq!(amplitude_to_blocks(0.25), "▂"); // 0.25 * 8 = 2.0 -> 2
        assert_eq!(amplitude_to_blocks(0.38), "▃"); // 0.38 * 8 = 3.04 -> 3
        assert_eq!(amplitude_to_blocks(0.5), "▄"); // 0.5 * 8 = 4.0 -> 4
        assert_eq!(amplitude_to_blocks(0.63), "▅"); // 0.63 * 8 = 5.04 -> 5
        assert_eq!(amplitude_to_blocks(0.75), "▆"); // 0.75 * 8 = 6.0 -> 6
        assert_eq!(amplitude_to_blocks(0.88), "▇"); // 0.88 * 8 = 7.04 -> 7
        assert_eq!(amplitude_to_blocks(1.0), "█"); // 1.0 * 8 = 8.0 -> 8
        assert_eq!(amplitude_to_blocks(1.5), "█"); // Should clamp to 1.0
        assert_eq!(amplitude_to_blocks(-0.9), "▇"); // Should use absolute value
    }
}

/// Generate oscilloscope-style waveform line
#[allow(dead_code)]
pub fn generate_waveform_line(samples: &[f32], width: usize, height: usize) -> Vec<String> {
    let mut lines = vec![String::new(); height];

    if samples.is_empty() || width == 0 {
        return lines;
    }

    // Downsample to fit width
    let step = samples.len() as f32 / width as f32;
    let display_samples: Vec<f32> = (0..width)
        .map(|i| {
            let start = (i as f32 * step) as usize;
            let end = ((i + 1) as f32 * step) as usize;
            let end = end.min(samples.len());

            // Take average of samples in this slice
            if start < end {
                samples[start..end].iter().sum::<f32>() / (end - start) as f32
            } else {
                0.0
            }
        })
        .collect();

    // Convert to terminal coordinates
    for (x, &sample) in display_samples.iter().enumerate() {
        // Map sample (-1.0 to 1.0) to terminal row (0 to height-1)
        let y = ((1.0 - sample) * 0.5 * (height - 1) as f32) as usize;
        let y = y.min(height - 1);

        if x < lines[y].len() {
            lines[y].replace_range(x..=x, "█");
        } else {
            // Pad with spaces if needed
            while lines[y].len() < x {
                lines[y].push(' ');
            }
            lines[y].push('█');
        }
    }

    lines
}
