//! .zimignore file parsing and pattern matching
//!
//! This module provides functionality to parse .zimignore files and check
//! whether files or directories should be ignored during zim update operations.
//! The syntax is similar to .gitignore with support for glob patterns.

use std::fs;
use std::io;
use std::path::Path;

/// A compiled .zimignore pattern
#[derive(Debug, Clone)]
pub struct IgnorePattern {
    pattern: String,
    is_directory: bool,
    is_negation: bool,
    is_absolute: bool,
}

impl IgnorePattern {
    /// Parse a single line from a .zimignore file
    pub fn parse(line: &str) -> Option<Self> {
        let line = line.trim();

        // Skip empty lines and comments
        if line.is_empty() || line.starts_with('#') {
            return None;
        }

        let (is_negation, pattern) = if let Some(stripped) = line.strip_prefix('!') {
            (true, stripped)
        } else {
            (false, line)
        };

        let is_directory = pattern.ends_with('/');
        let is_absolute = pattern.starts_with('/');

        // Clean up the pattern
        let pattern = pattern
            .trim_end_matches('/')
            .trim_start_matches('/')
            .to_string();

        Some(IgnorePattern {
            pattern,
            is_directory,
            is_negation,
            is_absolute,
        })
    }

    /// Check if this pattern matches the given path
    pub fn matches(&self, path: &Path, is_dir: bool) -> bool {
        // If pattern is directory-only but path is not a directory, no match
        if self.is_directory && !is_dir {
            return false;
        }

        let path_str = path.to_string_lossy();

        if self.is_absolute {
            // Absolute pattern - match from root
            self.glob_match(&self.pattern, &path_str)
        } else {
            // Relative pattern - match any component
            if self.glob_match(&self.pattern, &path_str) {
                return true;
            }

            // Also check individual path components
            for component in path.components() {
                let component_str = component.as_os_str().to_string_lossy();
                if self.glob_match(&self.pattern, &component_str) {
                    return true;
                }
            }

            // Check if pattern matches any suffix of the path
            let parts: Vec<&str> = path_str.split('/').collect();
            for i in 0..parts.len() {
                let suffix = parts[i..].join("/");
                if self.glob_match(&self.pattern, &suffix) {
                    return true;
                }
            }

            false
        }
    }

    /// Simple glob matching - supports * and **
    fn glob_match(&self, pattern: &str, text: &str) -> bool {
        Self::glob_match_recursive(pattern, text)
    }

    fn glob_match_recursive(pattern: &str, text: &str) -> bool {
        if pattern.is_empty() {
            return text.is_empty();
        }

        if pattern == "**" {
            return true; // ** matches everything
        }

        if let Some(rest_pattern) = pattern.strip_prefix("**/") {
            // **/ at start - match this pattern at any depth
            return Self::glob_match_recursive(rest_pattern, text)
                || text.contains('/') && {
                    let after_slash = text.split_once('/').map(|(_, after)| after).unwrap_or("");
                    Self::glob_match_recursive(pattern, after_slash)
                };
        }

        if let Some(prefix) = pattern.strip_suffix("/**") {
            // /**$ at end - match if text starts with the prefix
            return text.starts_with(prefix)
                && (text.len() == prefix.len() || text.chars().nth(prefix.len()) == Some('/'));
        }

        if let Some(star_pos) = pattern.find('*') {
            let before = &pattern[..star_pos];
            let after = &pattern[star_pos + 1..];

            if !text.starts_with(before) {
                return false;
            }

            let remaining_text = &text[before.len()..];

            // Try matching the rest at each position
            for i in 0..=remaining_text.len() {
                let candidate = &remaining_text[i..];
                if Self::glob_match_recursive(after, candidate) {
                    return true;
                }
            }
            false
        } else {
            // No wildcards - exact match
            pattern == text
        }
    }
}

/// A collection of ignore patterns from .zimignore files
#[derive(Debug, Default)]
pub struct ZimIgnore {
    patterns: Vec<IgnorePattern>,
}

impl ZimIgnore {
    /// Create a new empty ZimIgnore
    pub fn new() -> Self {
        Self::default()
    }

    /// Load patterns from a .zimignore file
    pub fn from_file<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        let content = fs::read_to_string(path)?;
        Ok(Self::from_content(&content))
    }

    /// Parse patterns from a string
    pub fn from_content(content: &str) -> Self {
        let patterns = content.lines().filter_map(IgnorePattern::parse).collect();

        Self { patterns }
    }

    /// Add patterns from another ZimIgnore (for hierarchical loading)
    pub fn extend(&mut self, other: &ZimIgnore) {
        self.patterns.extend(other.patterns.clone());
    }

    /// Check if a path should be ignored
    pub fn is_ignored(&self, path: &Path, is_dir: bool) -> bool {
        let mut should_ignore = false;

        // Process patterns in order - later patterns can override earlier ones
        for pattern in &self.patterns {
            if pattern.matches(path, is_dir) {
                should_ignore = !pattern.is_negation;
            }
        }

        should_ignore
    }

    /// Load .zimignore files hierarchically from a directory up to the root
    pub fn load_for_directory<P: AsRef<Path>>(dir: P) -> Self {
        let mut combined = ZimIgnore::new();
        let dir = dir.as_ref();

        // Walk up the directory tree looking for .zimignore files
        let mut current = Some(dir);
        let mut zimignore_files = Vec::new();

        while let Some(path) = current {
            let zimignore_path = path.join(".zimignore");
            if zimignore_path.exists() {
                zimignore_files.push(zimignore_path);
            }
            current = path.parent();
        }

        // Process files from root to current directory (reverse order)
        // This way more specific (deeper) rules override general ones
        for zimignore_path in zimignore_files.into_iter().rev() {
            if let Ok(zimignore) = ZimIgnore::from_file(&zimignore_path) {
                combined.extend(&zimignore);
            }
        }

        combined
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_pattern_parsing() {
        let pattern = IgnorePattern::parse("*.wav").unwrap();
        assert_eq!(pattern.pattern, "*.wav");
        assert!(!pattern.is_directory);
        assert!(!pattern.is_negation);
        assert!(!pattern.is_absolute);

        let pattern = IgnorePattern::parse("project/live/").unwrap();
        assert_eq!(pattern.pattern, "project/live");
        assert!(pattern.is_directory);

        let pattern = IgnorePattern::parse("!important.wav").unwrap();
        assert_eq!(pattern.pattern, "important.wav");
        assert!(pattern.is_negation);

        assert!(IgnorePattern::parse("# comment").is_none());
        assert!(IgnorePattern::parse("").is_none());
    }

    #[test]
    fn test_glob_matching() {
        let pattern = IgnorePattern::parse("*.wav").unwrap();
        assert!(pattern.matches(&PathBuf::from("test.wav"), false));
        assert!(!pattern.matches(&PathBuf::from("test.flac"), false));

        let pattern = IgnorePattern::parse("project/live/").unwrap();
        assert!(pattern.matches(&PathBuf::from("project/live"), true));
        assert!(!pattern.matches(&PathBuf::from("project/live"), false)); // Not a directory

        let pattern = IgnorePattern::parse("**/temp").unwrap();
        assert!(pattern.matches(&PathBuf::from("any/path/temp"), false));
        assert!(pattern.matches(&PathBuf::from("temp"), false));
    }

    #[test]
    fn test_zimignore_content() {
        let content = r#"
# Ignore DAW files
*.als
project/live/

# But keep important files
!important.als

# Ignore temp directories anywhere
**/temp/
"#;

        let zimignore = ZimIgnore::from_content(content);

        // Should ignore .als files
        assert!(zimignore.is_ignored(&PathBuf::from("song.als"), false));

        // But not the important one
        assert!(!zimignore.is_ignored(&PathBuf::from("important.als"), false));

        // Should ignore live directory
        assert!(zimignore.is_ignored(&PathBuf::from("project/live"), true));

        // Should ignore temp directories anywhere
        assert!(zimignore.is_ignored(&PathBuf::from("any/path/temp"), true));
        assert!(zimignore.is_ignored(&PathBuf::from("temp"), true));
    }

    #[test]
    fn test_absolute_vs_relative_patterns() {
        let pattern = IgnorePattern::parse("/project/live").unwrap();
        assert!(pattern.is_absolute);
        assert!(pattern.matches(&PathBuf::from("project/live"), true));
        assert!(!pattern.matches(&PathBuf::from("other/project/live"), true));

        let pattern = IgnorePattern::parse("project/live").unwrap();
        assert!(!pattern.is_absolute);
        assert!(pattern.matches(&PathBuf::from("project/live"), true));
        assert!(pattern.matches(&PathBuf::from("other/project/live"), true));
    }
}
