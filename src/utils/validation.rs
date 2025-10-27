//! Path and input validation utilities.
//!
//! This module provides common validation functions to ensure consistent
//! error handling across the codebase.

use owo_colors::OwoColorize;
use std::error::Error;
use std::path::Path;

/// Validate that a path exists and return an error if it doesn't.
///
/// # Arguments
///
/// * `path` - The path to validate
///
/// # Returns
///
/// * `Ok(())` if the path exists
/// * `Err` with a formatted error message if the path doesn't exist
///
/// # Example
///
/// ```ignore
/// use crate::utils::validation::validate_path_exists;
/// use std::path::Path;
///
/// let path = Path::new("/some/path");
/// validate_path_exists(path)?;
/// ```
pub fn validate_path_exists(path: &Path) -> Result<(), Box<dyn Error>> {
    if !path.exists() {
        return Err(format!(
            "{} Path does not exist: {}",
            "Error:".red().bold(),
            path.display()
        )
        .into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_validate_path_exists_valid() {
        let temp_dir = TempDir::new().unwrap();
        let result = validate_path_exists(temp_dir.path());
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_path_exists_invalid() {
        let path = Path::new("/this/path/does/not/exist/hopefully/12345");
        let result = validate_path_exists(path);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("does not exist"));
    }

    #[test]
    fn test_validate_path_exists_file() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        fs::write(&file_path, "test").unwrap();

        let result = validate_path_exists(&file_path);
        assert!(result.is_ok());
    }
}
