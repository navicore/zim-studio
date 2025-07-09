# update.rs Analysis and Refactoring Plan

## Overview
`cli/update.rs` handles creating and updating sidecar metadata files for audio files, totaling 284 lines. It recursively scans directories and generates `.md` files with metadata extracted from audio files.

## Public Interface

### Core Function
```rust
pub fn handle_update(project_path: &str) -> Result<(), Box<dyn Error>>
```

## Current Structure Analysis

### Strengths
- Clear separation between counting and processing phases
- Good progress feedback with indicatif
- Handles different audio formats appropriately
- Thread-safe counters for multi-file processing
- Nice colored output for user feedback

### Areas for Improvement

1. **Magic Values**
   - Hard-coded audio extensions array
   - Directory skip list duplicated (same as lint.rs)
   - Hard-coded spinner characters (same as lint.rs)
   - Progress bar template strings

2. **Code Duplication**
   - Directory traversal logic similar to lint.rs
   - Hidden file detection repeated
   - Directory skip logic duplicated

3. **Function Length**
   - `handle_update` (98 lines) - could extract result printing
   - `process_media_file` (85 lines) - complex metadata handling
   - `scan_directory` (44 lines) - reasonable but could be cleaner

4. **Error Handling**
   - Uses generic `Box<dyn Error>` throughout
   - Warning messages could be more consistent

## Refactoring Opportunities

### 1. Extract Constants
```rust
const AUDIO_EXTENSIONS: &[&str] = &["wav", "flac", "aiff", "mp3", "m4a"];
const SKIP_DIRECTORIES: &[&str] = &["node_modules", ".git", "temp"];
const SPINNER_CHARS: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
const SIDECAR_EXTENSION: &str = "md";
```

### 2. Create Shared Module
Since both lint.rs and update.rs share directory traversal logic:
```rust
// In a new module like `common/fs.rs`
pub fn should_skip_directory(name: &str) -> bool;
pub fn is_hidden_file(path: &Path) -> bool;
```

### 3. Extract Helper Functions
- `create_progress_bar(total: u64) -> ProgressBar`
- `print_update_summary(created: u32, updated: u32, skipped: u32)`
- `extract_audio_metadata(path: &Path) -> Result<SidecarContent, Error>`
- `format_file_metadata(metadata: &Metadata) -> (u64, Option<String>)`

### 4. Simplify File Processing
Break down `process_media_file` into:
- `check_sidecar_exists(path: &Path) -> bool`
- `create_sidecar_content(path: &Path) -> Result<String, Error>`
- `write_sidecar(path: &Path, content: &str) -> Result<(), Error>`

## Test Requirements

### Unit Tests Needed

1. **Path Tests**
   - `test_get_sidecar_path()`
   - `test_sidecar_path_special_chars()`
   - `test_sidecar_path_no_extension()`

2. **Extension Tests**
   - `test_is_audio_file()`
   - `test_audio_extensions_case_insensitive()`

3. **Counter Tests**
   - `test_count_audio_files_empty_dir()`
   - `test_count_audio_files_nested()`
   - `test_count_audio_files_skip_hidden()`

4. **Metadata Tests**
   - `test_format_file_metadata()`
   - `test_format_modified_time()`

### Integration Tests Needed
- Test with actual file system structure
- Test sidecar creation for different audio formats
- Test update behavior with existing sidecars
- Test concurrent file processing

## Implementation Priority

1. **High Priority**
   - Extract constants to reduce magic values
   - Add unit tests for pure functions
   - Extract shared directory traversal logic

2. **Medium Priority**
   - Refactor process_media_file into smaller functions
   - Improve error messages
   - Add more detailed progress reporting

3. **Low Priority**
   - Add dry-run mode
   - Add force update option
   - Performance optimizations for large directories

## Metrics

### Current State
- Lines of code: 284
- Public functions: 1
- Test coverage: 0%
- Largest function: 98 lines (handle_update)

### Target State
- Extract at least 6 helper functions
- Reduce largest function to <50 lines
- Add 12+ unit tests
- Share common code with lint.rs module