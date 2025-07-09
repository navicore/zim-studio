# lint.rs Analysis and Refactoring Plan

## Overview
`cli/lint.rs` validates YAML frontmatter in markdown sidecar files, totaling 284 lines. It recursively scans directories for `.md` files and checks schema compliance.

## Public Interface

### Core Function
```rust
pub fn handle_lint(project_path: &str) -> Result<(), Box<dyn Error>>
```

## Current Structure Analysis

### Strengths
- Clear separation between scanning and validation logic
- Good error handling with helpful messages
- Nice CLI output with colors and progress indicators
- Proper schema validation using serde

### Areas for Improvement

1. **Error Handling**
   - Uses generic `Box<dyn Error>` throughout
   - Could benefit from specific error types
   - Error message formatting is inline

2. **Function Length**
   - `validate_yaml_frontmatter` (52 lines) - complex error mapping
   - `handle_lint` (92 lines) - could extract result printing
   - `scan_directory` (46 lines) - reasonable but could be cleaner

3. **Code Duplication**
   - Directory skipping logic could be extracted
   - Error formatting patterns are repeated
   - Progress styling could be extracted

4. **Magic Values**
   - Hard-coded spinner characters
   - Directory names to skip ("node_modules", ".git", "temp")
   - Error message patterns in string matching

## Refactoring Opportunities

### 1. Extract Constants
```rust
const SPINNER_CHARS: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
const SKIP_DIRECTORIES: &[&str] = &["node_modules", ".git", "temp"];
const YAML_DELIMITER: &str = "---\n";
```

### 2. Create Error Types
```rust
enum LintError {
    MissingFrontmatter,
    InvalidFrontmatterFormat,
    SchemaValidation(String),
    DurationValidation(String),
}
```

### 3. Extract Helper Functions
- `create_progress_spinner() -> ProgressBar`
- `print_lint_summary(total: u32, valid: u32, invalid: u32)`
- `format_validation_error(error: &str) -> String`
- `should_skip_directory(name: &str) -> bool`

## Test Requirements

### Unit Tests Needed

1. **Validation Tests**
   - `test_validate_yaml_frontmatter_valid()`
   - `test_validate_yaml_frontmatter_missing()`
   - `test_validate_yaml_frontmatter_invalid_format()`
   - `test_validate_yaml_frontmatter_schema_errors()`

2. **Duration Field Tests**
   - `test_duration_field_number()`
   - `test_duration_field_unknown()`
   - `test_duration_field_invalid()`

3. **Sidecar Detection Tests**
   - `test_is_sidecar_file_valid()`
   - `test_is_sidecar_file_invalid()`
   - `test_is_sidecar_file_edge_cases()`

4. **Error Formatting Tests**
   - `test_format_missing_field_error()`
   - `test_format_type_error()`
   - `test_format_unknown_field_error()`

5. **Directory Filtering Tests**
   - `test_should_skip_directory()`
   - `test_hidden_file_detection()`

### Integration Tests Needed
- Test with actual file system structure
- Test recursive directory scanning
- Test error accumulation

## Implementation Priority

1. **High Priority**
   - Add unit tests for pure functions
   - Extract constants for maintainability
   - Extract error formatting logic

2. **Medium Priority**
   - Create specific error types
   - Extract progress spinner creation
   - Improve function modularity

3. **Low Priority**
   - Add more metadata field validations
   - Performance optimizations for large directories
   - Additional output formats (JSON, etc.)

## Metrics

### Current State
- Lines of code: 284
- Public functions: 1
- Test coverage: 0%
- Largest function: 92 lines (handle_lint)

### Target State
- Extract at least 5 helper functions
- Reduce largest function to <50 lines
- Add 15+ unit tests
- Zero hard-coded values