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

#[derive(Clone, Copy, PartialEq)]
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
