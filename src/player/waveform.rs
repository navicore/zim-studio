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
