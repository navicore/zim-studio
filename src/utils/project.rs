//! Project-related utility functions.
//!
//! This module provides functions for working with project structure,
//! including finding project roots and determining file types.

use std::path::Path;

/// Maximum depth to traverse when looking for project root
const MAX_PROJECT_TRAVERSAL_DEPTH: usize = 10;

/// Find the project root by looking for the nearest .zimignore file.
///
/// This function traverses up the directory tree from the given file path,
/// looking for a `.zimignore` file that marks the project root.
///
/// # Arguments
///
/// * `file_path` - Path to a file within the project
///
/// # Returns
///
/// * `Some(String)` - The project directory name if found
/// * `None` - If no project root is found within the traversal depth limit
///
/// # Example
///
/// ```ignore
/// // Given a file at: /home/user/projects/my-song/mixes/final.wav
/// // With .zimignore at: /home/user/projects/my-song/.zimignore
/// // Returns: Some("my-song")
/// let project_name = find_project_root(Path::new("/home/user/projects/my-song/mixes/final.wav"));
/// assert_eq!(project_name, Some("my-song".to_string()));
/// ```
pub fn find_project_root(file_path: &Path) -> Option<String> {
    // Start from the file's parent directory
    let mut current = file_path.parent();
    let mut depth = 0;

    while let Some(dir) = current {
        // Prevent excessive traversal
        if depth >= MAX_PROJECT_TRAVERSAL_DEPTH {
            // Reached maximum depth, stop searching to prevent infinite loops
            break;
        }
        depth += 1;

        let zimignore_path = dir.join(".zimignore");
        if zimignore_path.exists() {
            // Found a project root - return its directory name
            // If this is the current working directory ("."), get the actual directory name
            if dir == Path::new(".") {
                // Get the absolute path to get the real directory name
                if let Ok(abs_path) = std::env::current_dir() {
                    return abs_path
                        .file_name()
                        .and_then(|name| name.to_str())
                        .map(|s| s.to_string());
                }
            }

            return dir
                .file_name()
                .and_then(|name| name.to_str())
                .map(|s| s.to_string());
        }
        current = dir.parent();
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_find_project_root() {
        // Create a temporary directory structure
        let temp_dir = TempDir::new().unwrap();
        let project_dir = temp_dir.path().join("my-project");
        let mixes_dir = project_dir.join("mixes");
        let nested_dir = mixes_dir.join("old");

        fs::create_dir_all(&nested_dir).unwrap();
        fs::write(project_dir.join(".zimignore"), "# test").unwrap();

        let file_path = nested_dir.join("test.wav");
        fs::write(&file_path, "").unwrap();

        // Test finding project root
        let result = find_project_root(&file_path);
        assert_eq!(result, Some("my-project".to_string()));
    }

    #[test]
    fn test_find_project_root_no_zimignore() {
        // Test with no .zimignore
        let temp_dir = TempDir::new().unwrap();
        let orphan_dir = temp_dir.path().join("orphan");
        fs::create_dir_all(&orphan_dir).unwrap();
        let orphan_file = orphan_dir.join("test.wav");
        fs::write(&orphan_file, "").unwrap();

        let result = find_project_root(&orphan_file);
        assert_eq!(result, None);
    }

    #[test]
    fn test_find_project_root_nested() {
        // Create deeply nested structure
        let temp_dir = TempDir::new().unwrap();
        let project_dir = temp_dir.path().join("test-project");
        let mut nested = project_dir.join("a");
        for _ in 0..3 {
            nested = nested.join("nested");
        }

        fs::create_dir_all(&nested).unwrap();
        fs::write(project_dir.join(".zimignore"), "# test").unwrap();

        let file_path = nested.join("deep.wav");
        fs::write(&file_path, "").unwrap();

        let result = find_project_root(&file_path);
        assert_eq!(result, Some("test-project".to_string()));
    }
}
