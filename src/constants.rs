//! Project-wide constants used across multiple modules.
//!
//! This module centralizes constant definitions to avoid duplication and ensure
//! consistency across the codebase.

/// Spinner animation characters for progress indicators
pub const SPINNER_CHARS: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

/// File extension for sidecar metadata files
pub const SIDECAR_EXTENSION: &str = "md";

/// Directories to skip during file system traversal
pub const SKIP_DIRECTORIES: &[&str] = &["node_modules", ".git", "temp"];

/// Supported audio file extensions
pub const AUDIO_EXTENSIONS: &[&str] = &["wav", "flac", "aiff", "mp3", "m4a"];

/// YAML frontmatter delimiter
pub const YAML_DELIMITER: &str = "---\n";
