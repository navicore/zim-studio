//! Audio telemetry and observability system for debugging audio playback and format issues.
//!
//! This module provides real-time monitoring and structured logging for audio
//! processing parameters, specifically focusing on level meters, sample rates,
//! and audio format detection. It helps debug audio behavior by capturing
//! detailed metrics and providing configurable output formats.

use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::time::Instant;

/// Configuration for telemetry collection and output
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetryConfig {
    /// Enable/disable telemetry collection
    pub enabled: bool,
    /// Buffer size for historical data (number of samples to keep)
    pub buffer_size: usize,
    /// Minimum interval between telemetry captures (ms)
    pub capture_interval_ms: u64,
    /// Enable audio level debugging
    pub debug_audio_levels: bool,
    /// Enable audio format parameter tracking
    pub debug_format_info: bool,
    /// Output format: "json", "csv", "log"
    pub output_format: String,
    /// File path for telemetry output (optional)
    pub output_file: Option<String>,
}

impl Default for TelemetryConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            buffer_size: 1000,
            capture_interval_ms: 50, // 20Hz sampling
            debug_audio_levels: true,
            debug_format_info: true,
            output_format: "log".to_string(),
            output_file: None,
        }
    }
}

/// Audio level metrics for monitoring channel levels and smoothing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioLevelMetrics {
    /// Current decay factor (0.0-1.0)
    pub decay_factor: f32,
    /// Rate of change per sample
    pub rate_of_change: f32,
    /// Input level before slewing
    pub input_level: f32,
    /// Output level after slewing
    pub output_level: f32,
    /// Whether smoothing is actively applied
    pub is_smoothing: bool,
    /// Channel identifier (L/R)
    pub channel: String,
}

/// Audio format metrics for monitoring format and level calculation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioFormatMetrics {
    /// RMS calculation input samples count
    pub sample_count: usize,
    /// Raw RMS value before scaling
    pub raw_rms: f32,
    /// Scaled RMS value (after sqrt and gain)
    pub scaled_rms: f32,
    /// Final clamped level (0.0-1.0)
    pub final_level: f32,
    /// Audio format (mono/stereo)
    pub audio_format: String,
    /// Sample rate
    pub sample_rate: u32,
}

/// Combined telemetry snapshot for a single measurement
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetrySnapshot {
    /// Timestamp of measurement (seconds since start for serialization)
    pub timestamp_secs: f64,
    /// Playback state (playing/stopped)
    pub playback_state: String,
    /// Left channel audio level metrics
    pub left_channel: AudioLevelMetrics,
    /// Right channel audio level metrics  
    pub right_channel: AudioLevelMetrics,
    /// Audio format metrics
    pub format_info: AudioFormatMetrics,
    /// Current playback position (0.0-1.0)
    pub playback_position: f32,
}

/// Main telemetry collector
pub struct AudioTelemetry {
    config: TelemetryConfig,
    snapshots: VecDeque<TelemetrySnapshot>,
    last_capture: Instant,
    start_time: Instant,
}

impl AudioTelemetry {
    /// Create new telemetry collector with default config
    pub fn new() -> Self {
        Self::with_config(TelemetryConfig::default())
    }

    /// Create new telemetry collector with custom config
    pub fn with_config(config: TelemetryConfig) -> Self {
        let buffer_size = config.buffer_size;
        let now = Instant::now();
        Self {
            config,
            snapshots: VecDeque::with_capacity(buffer_size),
            last_capture: now,
            start_time: now,
        }
    }

    /// Update configuration at runtime
    pub fn update_config(&mut self, config: TelemetryConfig) {
        self.config = config;
        // Resize buffer if needed
        if self.snapshots.capacity() != self.config.buffer_size {
            let mut new_snapshots = VecDeque::with_capacity(self.config.buffer_size);
            while let Some(snapshot) = self.snapshots.pop_front() {
                if new_snapshots.len() < self.config.buffer_size {
                    new_snapshots.push_back(snapshot);
                } else {
                    break;
                }
            }
            self.snapshots = new_snapshots;
        }
    }

    /// Capture telemetry snapshot if interval has elapsed
    #[allow(clippy::too_many_arguments)]
    pub fn maybe_capture(
        &mut self,
        left_level: f32,
        right_level: f32,
        left_prev: f32,
        right_prev: f32,
        playback_state: &str,
        playback_position: f32,
        sample_data: Option<&[f32]>,
        audio_info: Option<&crate::player::audio::AudioInfo>,
    ) {
        if !self.config.enabled {
            return;
        }

        let now = Instant::now();
        let elapsed = now.duration_since(self.last_capture);

        if elapsed.as_millis() < self.config.capture_interval_ms as u128 {
            return;
        }

        let snapshot = self.create_snapshot(
            left_level,
            right_level,
            left_prev,
            right_prev,
            playback_state,
            playback_position,
            sample_data,
            audio_info,
            now,
        );

        self.add_snapshot(snapshot);
        self.last_capture = now;
    }

    /// Force capture a telemetry snapshot immediately
    #[allow(dead_code, clippy::too_many_arguments)]
    pub fn force_capture(
        &mut self,
        left_level: f32,
        right_level: f32,
        left_prev: f32,
        right_prev: f32,
        playback_state: &str,
        playback_position: f32,
        sample_data: Option<&[f32]>,
        audio_info: Option<&crate::player::audio::AudioInfo>,
    ) {
        if !self.config.enabled {
            return;
        }

        let snapshot = self.create_snapshot(
            left_level,
            right_level,
            left_prev,
            right_prev,
            playback_state,
            playback_position,
            sample_data,
            audio_info,
            Instant::now(),
        );

        self.add_snapshot(snapshot);
    }

    #[allow(clippy::too_many_arguments)]
    fn create_snapshot(
        &self,
        left_level: f32,
        right_level: f32,
        left_prev: f32,
        right_prev: f32,
        playback_state: &str,
        playback_position: f32,
        sample_data: Option<&[f32]>,
        audio_info: Option<&crate::player::audio::AudioInfo>,
        timestamp: Instant,
    ) -> TelemetrySnapshot {
        // Calculate audio level metrics
        let left_channel = AudioLevelMetrics {
            decay_factor: if playback_state == "playing" {
                0.99
            } else {
                0.0
            },
            rate_of_change: left_level - left_prev,
            input_level: left_prev,
            output_level: left_level,
            is_smoothing: (left_level - left_prev).abs() > 0.01,
            channel: "L".to_string(),
        };

        let right_channel = AudioLevelMetrics {
            decay_factor: if playback_state == "playing" {
                0.99
            } else {
                0.0
            },
            rate_of_change: right_level - right_prev,
            input_level: right_prev,
            output_level: right_level,
            is_smoothing: (right_level - right_prev).abs() > 0.01,
            channel: "R".to_string(),
        };

        // Calculate format metrics
        let format_info = if let Some(samples) = sample_data {
            let sample_count = samples.len();
            let raw_rms = if sample_count > 0 {
                let sum: f32 = samples.iter().map(|s| s * s).sum();
                (sum / sample_count as f32).sqrt()
            } else {
                0.0
            };
            let scaled_rms = raw_rms * 2.0; // Based on current scaling in code
            let final_level = scaled_rms.min(1.0);

            AudioFormatMetrics {
                sample_count,
                raw_rms,
                scaled_rms,
                final_level,
                audio_format: if let Some(info) = audio_info {
                    if info.channels > 1 {
                        "stereo".to_string()
                    } else {
                        "mono".to_string()
                    }
                } else {
                    "unknown".to_string()
                },
                sample_rate: audio_info.map(|i| i.sample_rate).unwrap_or(0),
            }
        } else {
            AudioFormatMetrics {
                sample_count: 0,
                raw_rms: 0.0,
                scaled_rms: 0.0,
                final_level: 0.0,
                audio_format: "unknown".to_string(),
                sample_rate: 0,
            }
        };

        TelemetrySnapshot {
            timestamp_secs: timestamp.duration_since(self.start_time).as_secs_f64(),
            playback_state: playback_state.to_string(),
            left_channel,
            right_channel,
            format_info,
            playback_position,
        }
    }

    fn add_snapshot(&mut self, snapshot: TelemetrySnapshot) {
        // Add to buffer, removing oldest if at capacity
        if self.snapshots.len() >= self.config.buffer_size {
            self.snapshots.pop_front();
        }
        self.snapshots.push_back(snapshot.clone());

        // Output based on format
        self.output_snapshot(&snapshot);
    }

    fn output_snapshot(&self, snapshot: &TelemetrySnapshot) {
        match self.config.output_format.as_str() {
            "json" => self.output_json(snapshot),
            "csv" => self.output_csv(snapshot),
            "log" => self.output_log(snapshot),
            _ => self.output_log(snapshot), // Default to log format
        }
    }

    fn output_json(&self, snapshot: &TelemetrySnapshot) {
        if let Ok(json) = serde_json::to_string(snapshot)
            && (self.config.debug_audio_levels || self.config.debug_format_info)
        {
            log::debug!("TELEMETRY_JSON: {json}");
        }
    }

    fn output_csv(&self, snapshot: &TelemetrySnapshot) {
        let csv_line = format!(
            "{:.3},{},{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},{},{},{:.3}",
            snapshot.timestamp_secs,
            snapshot.playback_state,
            snapshot.left_channel.input_level,
            snapshot.left_channel.output_level,
            snapshot.left_channel.rate_of_change,
            snapshot.right_channel.input_level,
            snapshot.right_channel.output_level,
            snapshot.right_channel.rate_of_change,
            snapshot.format_info.sample_count,
            snapshot.format_info.raw_rms,
            snapshot.playback_position
        );

        if self.config.debug_audio_levels || self.config.debug_format_info {
            log::debug!("TELEMETRY_CSV: {csv_line}");
        }
    }

    fn output_log(&self, snapshot: &TelemetrySnapshot) {
        if self.config.debug_audio_levels {
            log::debug!(
                "AUDIO_LEVELS: L[{:.3}->{:.3} Δ{:.3} smooth:{}] R[{:.3}->{:.3} Δ{:.3} smooth:{}] decay:{:.2}",
                snapshot.left_channel.input_level,
                snapshot.left_channel.output_level,
                snapshot.left_channel.rate_of_change,
                snapshot.left_channel.is_smoothing,
                snapshot.right_channel.input_level,
                snapshot.right_channel.output_level,
                snapshot.right_channel.rate_of_change,
                snapshot.right_channel.is_smoothing,
                snapshot.left_channel.decay_factor
            );
        }

        if self.config.debug_format_info {
            log::debug!(
                "FORMAT_INFO: samples:{} rms:{:.3} scaled:{:.3} final:{:.3} format:{} rate:{}Hz pos:{:.1}%",
                snapshot.format_info.sample_count,
                snapshot.format_info.raw_rms,
                snapshot.format_info.scaled_rms,
                snapshot.format_info.final_level,
                snapshot.format_info.audio_format,
                snapshot.format_info.sample_rate,
                snapshot.playback_position * 100.0
            );
        }
    }

    /// Get recent telemetry snapshots for analysis
    #[allow(dead_code)]
    pub fn get_recent_snapshots(&self, count: usize) -> Vec<&TelemetrySnapshot> {
        self.snapshots.iter().rev().take(count).collect()
    }

    /// Get all stored snapshots
    #[allow(dead_code)]
    pub fn get_all_snapshots(&self) -> &VecDeque<TelemetrySnapshot> {
        &self.snapshots
    }

    /// Clear all stored snapshots
    #[allow(dead_code)]
    pub fn clear(&mut self) {
        self.snapshots.clear();
    }

    /// Get current configuration
    pub fn config(&self) -> &TelemetryConfig {
        &self.config
    }

    /// Export snapshots to JSON string
    #[allow(dead_code)]
    pub fn export_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(&self.snapshots)
    }

    /// Export snapshots to CSV format
    #[allow(dead_code)]
    pub fn export_csv(&self) -> String {
        let mut csv = String::from(
            "timestamp,state,left_in,left_out,left_delta,right_in,right_out,right_delta,samples,rms,position\n",
        );

        for snapshot in &self.snapshots {
            csv.push_str(&format!(
                "{:.3},{},{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},{},{:.3},{:.3}\n",
                snapshot.timestamp_secs,
                snapshot.playback_state,
                snapshot.left_channel.input_level,
                snapshot.left_channel.output_level,
                snapshot.left_channel.rate_of_change,
                snapshot.right_channel.input_level,
                snapshot.right_channel.output_level,
                snapshot.right_channel.rate_of_change,
                snapshot.format_info.sample_count,
                snapshot.format_info.raw_rms,
                snapshot.playback_position
            ));
        }

        csv
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_telemetry_config_default() {
        let config = TelemetryConfig::default();
        assert!(!config.enabled);
        assert_eq!(config.buffer_size, 1000);
        assert_eq!(config.capture_interval_ms, 50);
        assert!(config.debug_audio_levels);
        assert!(config.debug_format_info);
        assert_eq!(config.output_format, "log");
        assert!(config.output_file.is_none());
    }

    #[test]
    fn test_telemetry_creation() {
        let telemetry = AudioTelemetry::new();
        assert_eq!(telemetry.snapshots.len(), 0);
        assert!(!telemetry.config.enabled);
    }

    #[test]
    fn test_telemetry_disabled_no_capture() {
        let mut telemetry = AudioTelemetry::new();
        // Config is disabled by default

        telemetry.maybe_capture(0.5, 0.6, 0.4, 0.5, "playing", 0.5, None, None);

        assert_eq!(telemetry.snapshots.len(), 0);
    }

    #[test]
    fn test_telemetry_enabled_capture() {
        let mut config = TelemetryConfig::default();
        config.enabled = true;
        config.capture_interval_ms = 0; // Immediate capture

        let mut telemetry = AudioTelemetry::with_config(config);

        telemetry.maybe_capture(0.5, 0.6, 0.4, 0.5, "playing", 0.5, None, None);

        assert_eq!(telemetry.snapshots.len(), 1);

        let snapshot = &telemetry.snapshots[0];
        assert_eq!(snapshot.playback_state, "playing");
        assert_eq!(snapshot.left_channel.output_level, 0.5);
        assert_eq!(snapshot.right_channel.output_level, 0.6);
    }

    #[test]
    fn test_slew_gate_metrics() {
        let mut config = TelemetryConfig::default();
        config.enabled = true;
        config.capture_interval_ms = 0;

        let mut telemetry = AudioTelemetry::with_config(config);

        // Test transition that should trigger limiting
        telemetry.force_capture(0.8, 0.9, 0.1, 0.2, "playing", 0.5, None, None);

        let snapshot = &telemetry.snapshots[0];
        assert!(snapshot.left_channel.is_smoothing); // Large change should trigger limiting
        assert!(snapshot.right_channel.is_smoothing);
        assert_eq!(snapshot.left_channel.rate_of_change, 0.7); // 0.8 - 0.1
        assert_eq!(snapshot.right_channel.rate_of_change, 0.7); // 0.9 - 0.2
    }

    #[test]
    fn test_buffer_size_limit() {
        let mut config = TelemetryConfig::default();
        config.enabled = true;
        config.buffer_size = 2;
        config.capture_interval_ms = 0;

        let mut telemetry = AudioTelemetry::with_config(config);

        // Add 3 snapshots to 2-item buffer
        for i in 0..3 {
            telemetry.force_capture(
                i as f32 * 0.1,
                i as f32 * 0.1,
                0.0,
                0.0,
                "playing",
                0.0,
                None,
                None,
            );
        }

        assert_eq!(telemetry.snapshots.len(), 2);
        // Should contain the last 2 snapshots (indices 1 and 2)
        assert_eq!(telemetry.snapshots[0].left_channel.output_level, 0.1);
        assert_eq!(telemetry.snapshots[1].left_channel.output_level, 0.2);
    }

    #[test]
    fn test_export_formats() {
        let mut config = TelemetryConfig::default();
        config.enabled = true;
        config.capture_interval_ms = 0;

        let mut telemetry = AudioTelemetry::with_config(config);

        telemetry.force_capture(0.5, 0.6, 0.4, 0.5, "playing", 0.5, None, None);

        // Test JSON export
        let json = telemetry.export_json().unwrap();
        assert!(json.contains("playback_state"));
        assert!(json.contains("playing"));

        // Test CSV export
        let csv = telemetry.export_csv();
        assert!(csv.contains("timestamp,state,left_in"));
        assert!(csv.contains("playing"));
    }
}
