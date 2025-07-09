//! Save dialog for exporting audio files and selections.
//!
//! This module provides a file browser dialog for saving audio files, with support
//! for navigating directories and editing filenames. It tracks whether the user is
//! saving a selection or the full file, and automatically generates appropriate
//! filenames for edits (e.g., "original_edit.wav", "original_edit_2.wav").

use std::fs;
use std::path::PathBuf;

#[derive(Clone)]
pub struct SaveDialog {
    pub current_path: PathBuf,
    pub filename: String,
    pub directories: Vec<String>,
    pub selected_index: usize,
    pub focus: SaveDialogFocus,
    pub has_selection: bool, // Whether we're saving a selection or full file
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SaveDialogFocus {
    DirectoryList,
    FilenameField,
}

impl SaveDialog {
    pub fn new(initial_path: PathBuf, suggested_filename: String, has_selection: bool) -> Self {
        let mut dialog = Self {
            current_path: initial_path.clone(),
            filename: suggested_filename,
            directories: Vec::new(),
            selected_index: 0,
            focus: SaveDialogFocus::DirectoryList,
            has_selection,
        };

        // Load directories for initial path
        dialog.refresh_directories();
        dialog
    }

    pub fn refresh_directories(&mut self) {
        self.directories.clear();

        // Add parent directory option if not at root
        if self.current_path.parent().is_some() {
            self.directories.push("..".to_string());
        }

        // Read current directory
        if let Ok(entries) = fs::read_dir(&self.current_path) {
            for entry in entries.flatten() {
                if let Ok(metadata) = entry.metadata() {
                    if metadata.is_dir() {
                        if let Some(name) = entry.file_name().to_str() {
                            // Skip hidden directories
                            if !name.starts_with('.') {
                                self.directories.push(name.to_string());
                            }
                        }
                    }
                }
            }
        }

        // Sort directories (but keep ".." at top)
        let has_parent = self.directories.first().map(|s| s == "..").unwrap_or(false);
        if has_parent {
            let mut dirs = self.directories.split_off(1);
            dirs.sort();
            self.directories.append(&mut dirs);
        } else {
            self.directories.sort();
        }

        // Reset selection
        self.selected_index = 0;
    }

    pub fn navigate_up(&mut self) {
        if self.selected_index > 0 {
            self.selected_index -= 1;
        }
    }

    pub fn navigate_down(&mut self) {
        if self.selected_index < self.directories.len().saturating_sub(1) {
            self.selected_index += 1;
        }
    }

    pub fn enter_directory(&mut self) {
        if let Some(dir_name) = self.directories.get(self.selected_index) {
            if dir_name == ".." {
                if let Some(parent) = self.current_path.parent() {
                    self.current_path = parent.to_path_buf();
                }
            } else {
                self.current_path = self.current_path.join(dir_name);
            }
            self.refresh_directories();
        }
    }

    pub fn toggle_focus(&mut self) {
        self.focus = match self.focus {
            SaveDialogFocus::DirectoryList => SaveDialogFocus::FilenameField,
            SaveDialogFocus::FilenameField => SaveDialogFocus::DirectoryList,
        };
    }

    pub fn push_char(&mut self, c: char) {
        if self.focus == SaveDialogFocus::FilenameField {
            self.filename.push(c);
        }
    }

    pub fn pop_char(&mut self) {
        if self.focus == SaveDialogFocus::FilenameField {
            self.filename.pop();
        }
    }

    pub fn get_full_path(&self) -> PathBuf {
        self.current_path.join(&self.filename)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_save_dialog_new() {
        let dialog = SaveDialog::new(PathBuf::from("/test/path"), "test.wav".to_string(), false);

        assert_eq!(dialog.current_path, PathBuf::from("/test/path"));
        assert_eq!(dialog.filename, "test.wav");
        assert_eq!(dialog.has_selection, false);
        assert_eq!(dialog.focus, SaveDialogFocus::DirectoryList);
        assert_eq!(dialog.selected_index, 0);
    }

    #[test]
    fn test_get_full_path() {
        let dialog = SaveDialog::new(
            PathBuf::from("/music/exports"),
            "track_edit.wav".to_string(),
            true,
        );

        assert_eq!(
            dialog.get_full_path(),
            PathBuf::from("/music/exports/track_edit.wav")
        );
    }

    #[test]
    fn test_refresh_directories() {
        let temp_dir = TempDir::new().unwrap();

        // Create some test directories
        fs::create_dir(temp_dir.path().join("visible_dir")).unwrap();
        fs::create_dir(temp_dir.path().join(".hidden_dir")).unwrap();
        fs::create_dir(temp_dir.path().join("another_dir")).unwrap();

        // Create a file (should not appear in directories)
        fs::write(temp_dir.path().join("test.txt"), "content").unwrap();

        let dialog = SaveDialog::new(
            temp_dir.path().to_path_buf(),
            "output.wav".to_string(),
            false,
        );

        // Should have parent dir (..) and visible directories
        assert!(dialog.directories.contains(&"..".to_string()));
        assert!(dialog.directories.contains(&"visible_dir".to_string()));
        assert!(dialog.directories.contains(&"another_dir".to_string()));

        // Should not have hidden directory or file
        assert!(!dialog.directories.contains(&".hidden_dir".to_string()));
        assert!(!dialog.directories.contains(&"test.txt".to_string()));
    }

    #[test]
    fn test_enter_directory_parent() {
        let mut dialog = SaveDialog::new(
            PathBuf::from("/parent/child"),
            "file.wav".to_string(),
            false,
        );

        // Select ".." and enter
        assert_eq!(dialog.directories[0], "..");
        dialog.selected_index = 0;
        dialog.enter_directory();

        assert_eq!(dialog.current_path, PathBuf::from("/parent"));
        // Selected index should reset after navigation
        assert_eq!(dialog.selected_index, 0);
    }

    #[test]
    fn test_enter_directory_subdirectory() {
        let temp_dir = TempDir::new().unwrap();
        let subdir = temp_dir.path().join("subdir");
        fs::create_dir(&subdir).unwrap();

        let mut dialog =
            SaveDialog::new(temp_dir.path().to_path_buf(), "file.wav".to_string(), false);

        // Find index of "subdir"
        let subdir_index = dialog
            .directories
            .iter()
            .position(|d| d == "subdir")
            .unwrap();
        dialog.selected_index = subdir_index;
        dialog.enter_directory();

        assert_eq!(dialog.current_path, subdir);
        assert_eq!(dialog.selected_index, 0);
    }

    #[test]
    fn test_directory_sorting() {
        let temp_dir = TempDir::new().unwrap();

        // Create directories with different names
        fs::create_dir(temp_dir.path().join("zebra")).unwrap();
        fs::create_dir(temp_dir.path().join("apple")).unwrap();
        fs::create_dir(temp_dir.path().join("banana")).unwrap();

        let dialog = SaveDialog::new(
            temp_dir.path().to_path_buf(),
            "output.wav".to_string(),
            false,
        );

        // Directories should be sorted alphabetically after ".."
        let expected_order = vec![
            "..".to_string(),
            "apple".to_string(),
            "banana".to_string(),
            "zebra".to_string(),
        ];
        assert_eq!(dialog.directories, expected_order);
    }

    #[test]
    fn test_toggle_focus() {
        let mut dialog = SaveDialog::new(PathBuf::from("/test"), "file.wav".to_string(), false);

        assert_eq!(dialog.focus, SaveDialogFocus::DirectoryList);

        dialog.toggle_focus();
        assert_eq!(dialog.focus, SaveDialogFocus::FilenameField);

        dialog.toggle_focus();
        assert_eq!(dialog.focus, SaveDialogFocus::DirectoryList);
    }

    #[test]
    fn test_push_pop_char() {
        let mut dialog = SaveDialog::new(PathBuf::from("/test"), "file".to_string(), false);

        // Should not add when focus is on directory list
        dialog.push_char('x');
        assert_eq!(dialog.filename, "file");

        // Switch focus to filename field
        dialog.toggle_focus();

        dialog.push_char('.');
        dialog.push_char('w');
        dialog.push_char('a');
        dialog.push_char('v');
        assert_eq!(dialog.filename, "file.wav");

        dialog.pop_char();
        assert_eq!(dialog.filename, "file.wa");

        dialog.pop_char();
        dialog.pop_char();
        assert_eq!(dialog.filename, "file.");
    }

    #[test]
    fn test_focus_enum() {
        assert_eq!(
            SaveDialogFocus::DirectoryList,
            SaveDialogFocus::DirectoryList
        );
        assert_ne!(
            SaveDialogFocus::DirectoryList,
            SaveDialogFocus::FilenameField
        );
    }
}
