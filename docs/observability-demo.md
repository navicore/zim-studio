# Audio Observability System Demo

## Overview
The ZIM Studio audio player now includes a comprehensive observability system for debugging slew gates (level decay) and VC control (voltage control/level calculation) parameters. This system provides real-time monitoring and structured logging of audio processing parameters.

## Quick Start

### 1. Launch the Player
```bash
cargo run --features player
```

### 2. Load an Audio File
- Press `b` to open the browser
- Navigate to an audio file (WAV or FLAC)
- Press Enter to load the file

### 3. Enable Telemetry
- Press `t` to enable telemetry debugging
- You'll see a log message: "Audio telemetry enabled - press 't' again to disable"

### 4. Observe the Debug Output
Watch the log file `/tmp/zim-player.log` for telemetry data:

```bash
tail -f /tmp/zim-player.log
```

## Understanding the Telemetry Data

### Slew Gate Monitoring
Slew gates control the rate of change in audio levels. The system monitors:

```
SLEW_GATES: L[0.450->0.445 Δ-0.005 limit:false] R[0.460->0.455 Δ-0.005 limit:false] decay:0.99
```

- **L/R**: Left and right channel data
- **Input->Output**: Level before and after slewing
- **Δ**: Rate of change (delta)
- **limit**: Whether the gate is actively limiting rapid changes
- **decay**: Current decay factor (0.99 when playing, 0.0 when stopped)

### VC Control Monitoring  
VC (Voltage Control) handles level calculation from audio samples:

```
VC_CONTROL: samples:1024 rms:0.123 scaled:0.246 final:0.246 format:stereo rate:44100Hz pos:23.5%
```

- **samples**: Number of audio samples processed
- **rms**: Raw RMS (Root Mean Square) value
- **scaled**: RMS after gain scaling (×2.0)
- **final**: Final clamped level (0.0-1.0)
- **format**: Audio format (mono/stereo)
- **rate**: Sample rate in Hz
- **pos**: Current playback position as percentage

## Key Behaviors to Debug

### 1. Level Decay (Slew Gates)
- **Playing**: Levels decay gradually with factor 0.99
- **Stopped**: Levels immediately drop to 0.0
- **Limiting**: Triggered when rate of change > 0.01

### 2. Audio Processing (VC Control)
- **Stereo**: Separate left/right channel processing
- **Mono**: Same level applied to both channels
- **RMS Calculation**: Real-time power measurement
- **Scaling**: 2x gain applied to RMS values
- **Clamping**: Final levels limited to 0.0-1.0 range

## Telemetry Configuration

### Enable with Custom Settings
```rust
let mut config = TelemetryConfig::default();
config.enabled = true;
config.capture_interval_ms = 50;  // 20Hz sampling
config.debug_slew_gates = true;
config.debug_vc_control = true;
config.output_format = "log";     // or "json", "csv"

app.enable_telemetry(config);
```

### Output Formats

#### Log Format (Human Readable)
```
SLEW_GATES: L[0.450->0.445 Δ-0.005 limit:false] R[0.460->0.455 Δ-0.005 limit:false] decay:0.99
VC_CONTROL: samples:1024 rms:0.123 scaled:0.246 final:0.246 format:stereo rate:44100Hz pos:23.5%
```

#### JSON Format (Machine Readable)
```json
{
  "timestamp_secs": 1.234,
  "playback_state": "playing",
  "left_slew": {
    "decay_factor": 0.99,
    "rate_of_change": -0.005,
    "input_level": 0.450,
    "output_level": 0.445,
    "is_limiting": false,
    "channel": "L"
  },
  "right_slew": { /* ... */ },
  "vc_control": { /* ... */ }
}
```

#### CSV Format (Data Analysis)
```csv
timestamp,state,left_in,left_out,left_delta,right_in,right_out,right_delta,samples,rms,position
1.234,playing,0.450,0.445,-0.005,0.460,0.455,-0.005,1024,0.123,0.235
```

## Common Debugging Scenarios

### 1. Investigating Level Meter Issues
Look for:
- Unexpected `is_limiting: true` when audio should be smooth
- Large rate of change values (Δ > 0.1)
- Inconsistent decay behavior between channels

### 2. Audio Processing Problems
Monitor:
- RMS calculation accuracy vs expected levels
- Scaling behavior (scaled should be 2x raw RMS)
- Sample count consistency with buffer sizes

### 3. Performance Analysis
Track:
- Capture intervals for consistent timing
- Sample processing rates
- Memory usage patterns in buffer management

## Keyboard Controls

| Key | Action |
|-----|--------|
| `t` | Toggle telemetry on/off |
| `space` | Play/pause (affects decay behavior) |
| `←/→` | Seek (triggers level changes) |
| `i/o` | Set marks (may affect telemetry capture) |
| `q` | Quit |

## Export Telemetry Data

```rust
// Export to JSON for analysis
let json_data = app.export_telemetry("json")?;
std::fs::write("telemetry.json", json_data)?;

// Export to CSV for spreadsheet analysis  
let csv_data = app.export_telemetry("csv")?;
std::fs::write("telemetry.csv", csv_data)?;
```

## Tips for Effective Debugging

1. **Start with known good audio** - Use a simple sine wave or familiar track
2. **Monitor during state changes** - Enable telemetry before play/pause/seek operations
3. **Compare left/right channels** - Look for asymmetric behavior in stereo content
4. **Watch decay patterns** - Smooth decay indicates healthy slew gate operation
5. **Correlate with visual meters** - Telemetry should match LED meter behavior

## Implementation Details

The observability system is designed to be:
- **Non-intrusive**: Minimal performance impact when disabled
- **Real-time**: Sub-100ms capture intervals for responsive debugging
- **Structured**: Machine-readable formats for automated analysis
- **Comprehensive**: Full parameter coverage for audio processing chain

For more details, see:
- `src/player/telemetry.rs` - Core telemetry implementation
- `src/player/app.rs` - Integration and keyboard controls
- Audio processing in `calculate_audio_levels()` and `apply_level_decay()`