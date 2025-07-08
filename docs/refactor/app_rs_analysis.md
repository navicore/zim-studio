# app.rs Analysis and Refactoring Plan

## Overview
`player/app.rs` is the main controller for the audio player, managing 675 lines of code with multiple responsibilities. This analysis identifies the public interface, refactoring opportunities, and test requirements.

## Public Interface

### Main Entry Points
- `App::run()` - Launches player without file
- `App::run_with_file(file_path: Option<&str>)` - Launches player with optional file

### Core App Methods
```rust
pub struct App {
    // State management for audio player
}

impl App {
    pub fn new() -> Self
    pub fn load_file(&mut self, path: &str) -> Result<(), Box<dyn Error>>
    pub fn toggle_playback(&mut self)
    pub fn update_waveform(&mut self)
    pub fn set_mark_in(&mut self)
    pub fn set_mark_out(&mut self)
    pub fn clear_marks(&mut self)
    pub fn toggle_loop(&mut self)
    pub fn get_selection_duration(&self) -> Option<Duration>
    pub fn open_save_dialog(&mut self)
    pub fn save_audio(&self, path: PathBuf, save_selection: bool) -> Result<(), Box<dyn Error>>
}
```

## Refactoring Opportunities

### 1. Extract Large Methods

#### `update_waveform()` (106 lines)
Should be split into:
- `update_audio_levels()` - Handle RMS calculation
- `update_playback_position()` - Handle position tracking
- `handle_loop_boundary()` - Handle loop logic
- `process_audio_samples()` - Handle sample processing

#### `run_app()` (125 lines)
Should be split into:
- `handle_keyboard_event()` - Process keyboard inputs
- `handle_browser_events()` - Handle browser-specific logic
- `handle_save_dialog_events()` - Handle save dialog logic
- `render_frame()` - Handle rendering

#### `save_wav_selection()` (62 lines)
Should be split into:
- `calculate_selection_samples()` - Calculate start/end samples
- `create_wav_writer()` - Setup WAV writer with specs
- `copy_audio_samples()` - Copy the actual audio data

### 2. Improve Error Handling
- Replace `Box<dyn Error>` with specific error types
- Create `PlayerError` enum for domain-specific errors
- Use `thiserror` crate for better error derivation

### 3. Separate Concerns
- Move file I/O operations to a separate `FileManager` struct
- Extract audio format conversion to `AudioConverter` trait
- Move UI event handling to `EventHandler` trait

### 4. State Management
- Consider using a state machine for playback states
- Extract mark/selection logic to `Selection` struct
- Use builder pattern for `SaveDialog` initialization

## Test Requirements

### Unit Tests Needed

1. **State Management Tests**
   - `test_new_app_initial_state()`
   - `test_load_file_updates_state()`
   - `test_playback_toggle_states()`
   - `test_mark_in_out_validation()`
   - `test_clear_marks()`
   - `test_loop_state_transitions()`

2. **Audio Processing Tests**
   - `test_waveform_buffer_updates()`
   - `test_audio_level_calculation()`
   - `test_playback_position_tracking()`
   - `test_selection_duration_calculation()`

3. **File Operations Tests**
   - `test_save_full_audio()`
   - `test_save_wav_selection()`
   - `test_save_flac_to_wav_conversion()`
   - `test_save_dialog_filename_generation()`

4. **Error Handling Tests**
   - `test_load_nonexistent_file()`
   - `test_save_to_invalid_path()`
   - `test_invalid_selection_range()`

### Integration Tests Needed

1. **End-to-End Player Tests**
   - Load file → Play → Mark → Save selection
   - Browser navigation → File selection → Playback
   - Loop playback with marks

2. **UI Event Tests**
   - Keyboard navigation through all screens
   - Save dialog interaction flow
   - Browser search and selection

## Code Quality Improvements

### 1. Documentation
- Add module-level documentation explaining the player architecture
- Document the state flow and event handling
- Add examples for common usage patterns

### 2. Type Safety
- Replace `f32` positions with `Position(f32)` newtype
- Use `Duration` consistently instead of `f32` seconds
- Create types for `MarkIn` and `MarkOut`

### 3. Performance
- Consider lazy loading for waveform visualization
- Optimize sample processing with iterators
- Add benchmarks for audio processing paths

## Implementation Priority

1. **High Priority**
   - Extract large methods (update_waveform, run_app)
   - Add basic unit tests for public API
   - Improve error handling

2. **Medium Priority**
   - Separate file I/O concerns
   - Add integration tests
   - Improve type safety

3. **Low Priority**
   - Performance optimizations
   - Advanced state machine
   - Comprehensive benchmarks