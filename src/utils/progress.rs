//! Progress bar utilities for creating consistent progress indicators across commands.
//!
//! This module provides standardized progress bar and spinner creation functions
//! to ensure consistent user experience across all CLI commands.

use crate::constants::SPINNER_CHARS;
use indicatif::{ProgressBar, ProgressStyle};

/// Create a standard progress spinner with consistent styling.
///
/// # Returns
///
/// A configured `ProgressBar` instance in spinner mode with cyan styling
/// and the standard spinner character sequence.
///
/// # Example
///
/// ```ignore
/// use crate::utils::progress::create_progress_spinner;
///
/// let spinner = create_progress_spinner();
/// spinner.set_message("Scanning files...");
/// // ... do work ...
/// spinner.finish_and_clear();
/// ```
pub fn create_progress_spinner() -> ProgressBar {
    let spinner = ProgressBar::new_spinner();
    spinner.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.cyan} {msg}")
            .unwrap()
            .tick_strings(SPINNER_CHARS),
    );
    spinner
}

/// Create a standard progress bar with consistent styling.
///
/// # Arguments
///
/// * `total` - The total number of items to process
///
/// # Returns
///
/// A configured `ProgressBar` instance with a cyan/blue color scheme
/// and standard progress bar formatting.
///
/// # Example
///
/// ```ignore
/// use crate::utils::progress::create_progress_bar;
///
/// let pb = create_progress_bar(100);
/// for i in 0..100 {
///     // ... do work ...
///     pb.inc(1);
/// }
/// pb.finish_with_message("Done");
/// ```
pub fn create_progress_bar(total: u64) -> ProgressBar {
    let pb = ProgressBar::new(total);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.cyan} [{bar:40.cyan/blue}] {pos}/{len} {msg}")
            .unwrap()
            .progress_chars("█▓░"),
    );
    pb
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_progress_spinner() {
        let spinner = create_progress_spinner();
        // Just verify it creates without panicking
        spinner.set_message("Test message");
        spinner.finish_and_clear();
    }

    #[test]
    fn test_create_progress_bar() {
        let pb = create_progress_bar(100);
        // Just verify it creates without panicking
        pb.set_position(50);
        pb.finish();
    }
}
