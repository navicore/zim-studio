# ui.rs Analysis and Refactoring Plan

## Overview
`player/ui.rs` handles terminal UI rendering for the audio player, totaling 478 lines. It manages waveform visualization, LED meters, progress bars, and adaptive layouts based on terminal size.

## Public Interface

### Core UI Functions
```rust
pub fn draw(f: &mut Frame, app: &App)
```

## Current Structure Analysis

### Strengths
- Clear separation between different UI components
- Adaptive layout based on terminal size
- Good use of ratatui's canvas for waveform visualization
- Clean color coding for different UI states

### Areas for Improvement

1. **Large Functions**
   - `draw_main_ui` (54 lines) - main layout orchestration
   - `draw_waveform` (42 lines) - oscilloscope rendering
   - `draw_audio_info` (38 lines) - info and LED display
   - `draw_progress_bar` (40 lines) - progress with marks
   - `draw_controls` (33 lines) - control hints

2. **Code Duplication**
   - Similar pattern for creating styled spans
   - Repeated LED color calculations
   - Multiple instances of percentage formatting

3. **Magic Numbers**
   - Hard-coded layout constraints
   - Fixed color values for LEDs
   - Arbitrary thresholds for adaptive display

## Refactoring Opportunities

### 1. Extract Constants
```rust
const MIN_HEIGHT_FOR_OSCILLOSCOPE: u16 = 20;
const LED_COLORS: [(f32, Color); 5] = [
    (0.9, Color::Red),
    (0.7, Color::LightRed),
    (0.5, Color::Yellow),
    (0.3, Color::Green),
    (0.0, Color::DarkGray),
];
```

### 2. Create Helper Functions
- `create_styled_span(text: &str, style: Style) -> Span`
- `format_percentage(value: f32) -> String`
- `get_led_color(level: f32) -> Color`
- `create_title_block(title: &str) -> Block`

### 3. Simplify Layout Creation
Extract layout configuration into data structures:
```rust
struct LayoutConfig {
    show_oscilloscope: bool,
    constraints: Vec<Constraint>,
}
```

## Test Requirements

### Unit Tests Needed

1. **LED Color Tests**
   - `test_get_led_color_ranges()`
   - `test_led_color_boundaries()`

2. **Formatting Tests**
   - `test_format_time()`
   - `test_format_percentage()`
   - `test_format_duration()`

3. **Layout Tests**
   - `test_layout_constraints_with_oscilloscope()`
   - `test_layout_constraints_without_oscilloscope()`
   - `test_adaptive_display_thresholds()`

4. **Mark Position Tests**
   - `test_calculate_mark_position()`
   - `test_mark_position_boundaries()`

### UI Component Tests
Since most functions take `Frame` which is hard to test directly, we should:
1. Extract pure logic into testable functions
2. Test calculations and formatting separately from rendering
3. Create helper structs for testable UI state

## Implementation Priority

1. **High Priority**
   - Extract constants for colors and thresholds
   - Add unit tests for pure functions
   - Extract LED color calculation logic

2. **Medium Priority**
   - Refactor large drawing functions
   - Create reusable UI component builders
   - Improve mark rendering logic

3. **Low Priority**
   - Create theme system for colors
   - Add more visualization options
   - Performance optimizations

## Metrics

### Current State
- Lines of code: 478
- Public functions: 1
- Test coverage: 0%
- Largest function: 54 lines (draw_main_ui)

### Target State
- Extract at least 5 helper functions
- Reduce largest function to <30 lines
- Add 10+ unit tests for logic
- Zero magic numbers