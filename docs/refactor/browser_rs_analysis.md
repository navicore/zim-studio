# browser.rs Analysis and Refactoring Plan

## Overview
`player/browser.rs` implements a telescope-style file browser for audio discovery, totaling 490 lines. It searches markdown sidecar files for metadata while displaying the actual audio files.

## Public Interface

### Core Browser Methods
```rust
impl Browser {
    pub fn new() -> Self
    pub fn scan_directory(&mut self, path: &Path) -> Result<(), Box<dyn std::error::Error>>
    pub fn toggle(&mut self)
    pub fn push_char(&mut self, c: char)
    pub fn pop_char(&mut self)
    pub fn clear_search(&mut self)  // marked as dead_code
    pub fn select_next(&mut self)
    pub fn select_previous(&mut self)
    pub fn get_selected_path(&self) -> Option<&Path>
}

// UI Function
pub fn draw_browser(f: &mut Frame, area: Rect, browser: &Browser)
```

## Current Structure Analysis

### Strengths
- Clear separation between data model and UI rendering
- Efficient substring search implementation
- Good handling of recursive directory scanning
- Clean metadata parsing from sidecar files

### Areas for Improvement

1. **Error Handling**
   - Uses generic `Box<dyn std::error::Error>` throughout
   - Could benefit from specific error types

2. **Large Functions**
   - `draw_browser` (158 lines) - could be broken down
   - `filter_items` (71 lines) - complex scoring logic
   - `scan_directory_recursive` (68 lines) - file type handling

3. **Code Duplication**
   - Similar pattern matching for file extensions appears twice
   - Context extraction could be reused

## Refactoring Opportunities

### 1. Extract UI Components
Break down `draw_browser` into smaller functions:
- `draw_search_input`
- `draw_results_list`
- `draw_help_bar`
- `draw_preview_pane`

### 2. Simplify filter_items
Extract scoring logic:
```rust
fn score_item(item: &AudioFile, query: &str) -> Option<(i64, Option<String>)>
fn find_content_match(content: &str, query: &str) -> Option<(i64, String)>
fn find_filename_match(filename: &str, query: &str) -> Option<i64>
```

### 3. File Type Handling
Create a more extensible approach:
```rust
const SUPPORTED_AUDIO_EXTENSIONS: &[&str] = &["wav", "flac"];

fn is_supported_audio_file(path: &Path) -> bool
```

## Test Requirements

### Unit Tests Needed

1. **Initialization Tests**
   - `test_new_browser()`
   - `test_browser_initial_state()`

2. **Directory Scanning Tests**
   - `test_scan_empty_directory()`
   - `test_scan_directory_with_audio_files()`
   - `test_scan_directory_recursive()`
   - `test_scan_directory_skip_hidden()`
   - `test_scan_directory_with_sidecars()`

3. **Search Tests**
   - `test_search_empty_query()`
   - `test_search_by_content()`
   - `test_search_by_filename()`
   - `test_search_by_tags()`
   - `test_search_case_insensitive()`
   - `test_search_scoring()`

4. **Navigation Tests**
   - `test_select_next_previous()`
   - `test_select_wraparound()`
   - `test_get_selected_path()`

5. **Metadata Parsing Tests**
   - `test_parse_sidecar_with_title()`
   - `test_parse_sidecar_with_tags()`
   - `test_parse_sidecar_empty()`

6. **UI State Tests**
   - `test_toggle_browser()`
   - `test_push_pop_char()`
   - `test_clear_search()`

### Helper Functions Needed
- Mock file system setup for tests
- Test data generators for audio files and sidecars

## Implementation Priority

1. **High Priority**
   - Add comprehensive unit tests
   - Refactor `draw_browser` into smaller functions
   - Extract file type constants

2. **Medium Priority**
   - Simplify `filter_items` logic
   - Improve error types
   - Add more metadata field support

3. **Low Priority**
   - Performance optimizations for large directories
   - Additional search algorithms
   - Caching for repeated searches

## Metrics

### Current State
- Lines of code: 490
- Public methods: 9
- Test coverage: 0%
- Largest function: 158 lines (draw_browser)

### Target State
- Reduce largest function to <50 lines
- Achieve 80%+ test coverage
- Extract at least 5 helper functions
- Zero clippy warnings