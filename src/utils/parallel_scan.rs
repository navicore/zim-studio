//! Parallel directory scanning utilities for improved performance on large projects.
//!
//! This module provides parallelized directory traversal using rayon to significantly
//! speed up operations when processing directories with many files.
//!
//! # Note on Ordering
//!
//! When parallel processing is enabled (multiple subdirectories), the order of results
//! is non-deterministic. This is acceptable for audio file collection where order doesn't
//! matter, but should be considered if used for other purposes.

use crate::zimignore::ZimIgnore;
use rayon::prelude::*;
use std::collections::HashSet;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

const SKIP_DIRECTORIES: &[&str] = &["node_modules", ".git", "temp"];

/// Check if a file or directory is hidden (starts with '.')
pub fn is_hidden_file(path: &Path) -> bool {
    path.file_name()
        .map(|name| name.to_string_lossy().starts_with('.'))
        .unwrap_or(false)
}

/// Check if a directory should be skipped during traversal
pub fn should_skip_directory(name: &str) -> bool {
    SKIP_DIRECTORIES.contains(&name)
}

/// Collect all files matching the given extensions in a directory tree.
///
/// This function recursively scans directories in parallel for better performance
/// on large directory structures.
///
/// # Error Handling
///
/// Errors encountered while scanning subdirectories (e.g., permission denied, I/O errors)
/// are logged to stderr but do not stop the scan. This allows the function to continue
/// processing accessible directories and return all files that could be read.
///
/// This behavior is consistent across both parallel and sequential code paths.
///
/// # Arguments
///
/// * `dir` - Root directory to start scanning from
/// * `audio_exts` - Set of file extensions to match (without leading dot)
/// * `zimignore` - ZimIgnore instance for filtering ignored paths
///
/// # Returns
///
/// Vector of PathBufs for all matching files found in accessible directories
pub fn collect_audio_files(
    dir: &Path,
    audio_exts: &HashSet<&str>,
    zimignore: &ZimIgnore,
) -> Result<Vec<PathBuf>, Box<dyn Error>> {
    let mut files = Vec::new();
    scan_directory_parallel(dir, audio_exts, zimignore, &mut files)?;
    Ok(files)
}

/// Recursively scan a directory and collect audio files in parallel.
///
/// Uses rayon for parallel processing of subdirectories to improve performance.
fn scan_directory_parallel(
    dir: &Path,
    audio_exts: &HashSet<&str>,
    zimignore: &ZimIgnore,
    files: &mut Vec<PathBuf>,
) -> Result<(), Box<dyn Error>> {
    let entries = fs::read_dir(dir)?;

    // Collect entries first to enable parallel processing
    let entries: Vec<_> = entries.collect::<Result<_, _>>()?;

    // Separate files and directories for different handling
    let mut local_files = Vec::new();
    let mut directories = Vec::new();

    for entry in entries {
        let path = entry.path();

        // Skip hidden files and directories
        if is_hidden_file(&path) {
            continue;
        }

        // Check if this path should be ignored by .zimignore
        if zimignore.is_ignored(&path, path.is_dir()) {
            continue;
        }

        if path.is_dir() {
            // Use safer path handling instead of unwrap()
            let dir_name = match path.file_name() {
                Some(name) => name.to_string_lossy(),
                None => continue, // Skip paths without a valid file name
            };
            if !should_skip_directory(&dir_name) {
                directories.push(path);
            }
        } else if path.is_file()
            && let Some(extension) = path.extension()
        {
            let ext = extension.to_string_lossy().to_lowercase();
            if audio_exts.contains(ext.as_str()) {
                local_files.push(path);
            }
        }
    }

    // Add files from current directory
    files.extend(local_files);

    // Process subdirectories in parallel if there are multiple directories
    if directories.len() > 1 {
        // Parallel processing for multiple directories
        // Note: Errors during parallel traversal are logged to stderr but don't halt
        // the entire scan. This allows processing of accessible directories to continue
        // even if some encounter permission issues or network problems.
        let nested_files: Vec<Vec<PathBuf>> = directories
            .par_iter()
            .filter_map(
                |subdir| match collect_audio_files(subdir, audio_exts, zimignore) {
                    Ok(files) => Some(files),
                    Err(e) => {
                        eprintln!(
                            "Warning: Failed to scan directory '{}': {}",
                            subdir.display(),
                            e
                        );
                        None
                    }
                },
            )
            .collect();

        // Merge results
        for nested in nested_files {
            files.extend(nested);
        }
    } else {
        // Sequential processing for single directory (avoid overhead)
        // Error handling is consistent with parallel path: log errors but continue
        for subdir in directories {
            if let Err(e) = scan_directory_parallel(&subdir, audio_exts, zimignore, files) {
                eprintln!(
                    "Warning: Failed to scan directory '{}': {}",
                    subdir.display(),
                    e
                );
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_is_hidden_file() {
        assert!(is_hidden_file(Path::new(".hidden")));
        assert!(is_hidden_file(Path::new("/path/.hidden")));
        assert!(!is_hidden_file(Path::new("visible")));
    }

    #[test]
    fn test_should_skip_directory() {
        assert!(should_skip_directory("node_modules"));
        assert!(should_skip_directory(".git"));
        assert!(!should_skip_directory("src"));
    }

    #[test]
    fn test_collect_audio_files_empty() {
        let temp_dir = TempDir::new().unwrap();
        let audio_exts: HashSet<&str> = ["wav", "flac"].iter().cloned().collect();
        let zimignore = ZimIgnore::new();

        let files = collect_audio_files(temp_dir.path(), &audio_exts, &zimignore).unwrap();
        assert_eq!(files.len(), 0);
    }

    #[test]
    fn test_collect_audio_files_with_audio() {
        let temp_dir = TempDir::new().unwrap();
        let audio_exts: HashSet<&str> = ["wav", "flac"].iter().cloned().collect();

        // Create some audio files
        fs::write(temp_dir.path().join("test1.wav"), b"fake").unwrap();
        fs::write(temp_dir.path().join("test2.flac"), b"fake").unwrap();
        fs::write(temp_dir.path().join("readme.txt"), b"fake").unwrap();

        let zimignore = ZimIgnore::new();
        let files = collect_audio_files(temp_dir.path(), &audio_exts, &zimignore).unwrap();
        assert_eq!(files.len(), 2);
    }

    #[test]
    fn test_collect_audio_files_nested() {
        let temp_dir = TempDir::new().unwrap();
        let audio_exts: HashSet<&str> = ["wav"].iter().cloned().collect();

        // Create nested structure
        let subdir = temp_dir.path().join("music");
        fs::create_dir(&subdir).unwrap();
        fs::write(temp_dir.path().join("root.wav"), b"fake").unwrap();
        fs::write(subdir.join("nested.wav"), b"fake").unwrap();

        let zimignore = ZimIgnore::new();
        let files = collect_audio_files(temp_dir.path(), &audio_exts, &zimignore).unwrap();
        assert_eq!(files.len(), 2);
    }

    #[test]
    fn test_collect_audio_files_skip_hidden() {
        let temp_dir = TempDir::new().unwrap();
        let audio_exts: HashSet<&str> = ["wav"].iter().cloned().collect();

        fs::write(temp_dir.path().join("visible.wav"), b"fake").unwrap();
        fs::write(temp_dir.path().join(".hidden.wav"), b"fake").unwrap();

        let zimignore = ZimIgnore::new();
        let files = collect_audio_files(temp_dir.path(), &audio_exts, &zimignore).unwrap();
        assert_eq!(files.len(), 1);
    }

    #[test]
    fn test_collect_audio_files_skip_directories() {
        let temp_dir = TempDir::new().unwrap();
        let audio_exts: HashSet<&str> = ["wav"].iter().cloned().collect();

        // Create normal directory
        let normal_dir = temp_dir.path().join("music");
        fs::create_dir(&normal_dir).unwrap();
        fs::write(normal_dir.join("test.wav"), b"fake").unwrap();

        // Create skip directory
        let skip_dir = temp_dir.path().join("node_modules");
        fs::create_dir(&skip_dir).unwrap();
        fs::write(skip_dir.join("test.wav"), b"fake").unwrap();

        let zimignore = ZimIgnore::new();
        let files = collect_audio_files(temp_dir.path(), &audio_exts, &zimignore).unwrap();
        assert_eq!(files.len(), 1);
    }
}
