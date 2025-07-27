//! Application configuration management.
//!
//! This module handles the persistent configuration for zim-studio, including
//! the root directory for projects, default folders to create, artist information,
//! and various preferences. Configuration is stored in the user's config directory
//! (typically ~/.config/zim/config.toml) and supports customization of project
//! structure and behavior.

use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    #[serde(default = "default_artist")]
    pub default_artist: String,
    #[serde(default = "default_folders")]
    pub default_folders: Vec<String>,
    #[serde(default = "default_gitignore")]
    pub default_gitignore: Vec<String>,
    #[serde(default = "default_include_readmes")]
    pub include_readmes: bool,
    #[serde(default = "default_normalize_project_names")]
    pub normalize_project_names: bool,
    #[serde(default = "default_daw_folders")]
    pub daw_folders: Vec<String>,
}

fn default_artist() -> String {
    // Try to get username and capitalize first letter
    std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME")) // Windows fallback
        .ok()
        .and_then(|name| {
            if name.is_empty() {
                None
            } else {
                let mut chars = name.chars();
                chars.next().map(|first| {
                    first.to_uppercase().collect::<String>() + &chars.as_str().to_lowercase()
                })
            }
        })
        .unwrap_or_default()
}

fn default_folders() -> Vec<String> {
    vec![
        "sources".to_string(),
        "edits".to_string(),
        "bounced".to_string(),
        "mixes".to_string(),
        "masters".to_string(),
        "project".to_string(),
    ]
}

fn default_gitignore() -> Vec<String> {
    vec![
        // Audio files
        "*.wav".to_string(),
        "*.flac".to_string(),
        "*.aiff".to_string(),
        "*.aif".to_string(),
        "*.asd".to_string(),
        "*.mp3".to_string(),
        "*.m4a".to_string(),
        // Visual media files
        "*.jpg".to_string(),
        "*.jpeg".to_string(),
        "*.png".to_string(),
        "*.gif".to_string(),
        "*.mp4".to_string(),
        "*.mov".to_string(),
        "*.avi".to_string(),
        "*.webm".to_string(),
        "*.tiff".to_string(),
        "*.bmp".to_string(),
        "*.heic".to_string(),
        "*.heif".to_string(),
        // DAW temp files
        "*.als~".to_string(),
        "project/*/temp/".to_string(),
    ]
}

fn default_include_readmes() -> bool {
    true
}

fn default_normalize_project_names() -> bool {
    true
}

fn default_daw_folders() -> Vec<String> {
    vec![
        "live".to_string(),
        "reaper".to_string(),
        "bitwig".to_string(),
        "renoise".to_string(),
    ]
}

fn default_zimignore_content() -> String {
    r#"# ZIM Studio Default .zimignore
# 
# This file defines patterns for files and directories that should be ignored
# when running "zim update" to generate sidecar files. The syntax is similar
# to .gitignore with support for glob patterns.

# DAW Project Files and Directories
project/live/
project/reaper/
project/bitwig/
project/renoise/

# DAW-specific temporary and backup files
*.als-backup
*.rpp-bak
*.bwproject-backup
*.xrns-backup

# Common DAW auto-save and backup directories
**/Backup/
**/Auto Save/
**/AutoSave/
**/Ableton Project Info/

# Cache and temporary directories
**/.cache/
**/temp/
**/tmp/
**/.tmp/

# System files
.DS_Store
Thumbs.db
desktop.ini

# Log files
*.log

# Example: Keep important files even if they match patterns above
# Use ! to negate patterns
# !important.als
# !project/live/important-session.als
"#
    .to_string()
}

impl Default for Config {
    fn default() -> Self {
        Self::new()
    }
}

impl Config {
    pub fn new() -> Self {
        Self {
            default_artist: default_artist(),
            default_folders: default_folders(),
            default_gitignore: default_gitignore(),
            include_readmes: default_include_readmes(),
            normalize_project_names: default_normalize_project_names(),
            daw_folders: default_daw_folders(),
        }
    }

    pub fn config_dir() -> Result<PathBuf, Box<dyn Error>> {
        // Check for XDG_CONFIG_HOME first (useful for testing)
        let config_dir = if let Ok(xdg_config) = std::env::var("XDG_CONFIG_HOME") {
            PathBuf::from(xdg_config).join("zim")
        } else {
            dirs::config_dir()
                .ok_or("Unable to find config directory")?
                .join("zim")
        };
        Ok(config_dir)
    }

    pub fn config_path() -> Result<PathBuf, Box<dyn Error>> {
        Ok(Self::config_dir()?.join("config.toml"))
    }

    pub fn default_zimignore_path() -> Result<PathBuf, Box<dyn Error>> {
        Ok(Self::config_dir()?.join("default.zimignore"))
    }

    pub fn load() -> Result<Self, Box<dyn Error>> {
        let config_path = Self::config_path()?;

        if !config_path.exists() {
            // Return default config instead of error
            return Ok(Default::default());
        }

        let contents = fs::read_to_string(&config_path)?;
        let config: Config = toml::from_str(&contents)?;
        Ok(config)
    }

    pub fn save(&self) -> Result<(), Box<dyn Error>> {
        let config_dir = Self::config_dir()?;

        if !config_dir.exists() {
            fs::create_dir_all(&config_dir)?;
        }

        let config_path = Self::config_path()?;
        let toml_string = toml::to_string_pretty(self)?;
        fs::write(&config_path, toml_string)?;

        Ok(())
    }

    pub fn exists() -> Result<bool, Box<dyn Error>> {
        Ok(Self::config_path()?.exists())
    }

    /// Create the default .zimignore template if it doesn't exist
    pub fn ensure_default_zimignore() -> Result<(), Box<dyn Error>> {
        let zimignore_path = Self::default_zimignore_path()?;

        if !zimignore_path.exists() {
            let config_dir = Self::config_dir()?;
            if !config_dir.exists() {
                fs::create_dir_all(&config_dir)?;
            }

            let default_content = default_zimignore_content();
            fs::write(&zimignore_path, default_content)?;
        }

        Ok(())
    }

    /// Load the default .zimignore template content
    pub fn load_default_zimignore() -> Result<String, Box<dyn Error>> {
        let zimignore_path = Self::default_zimignore_path()?;

        if zimignore_path.exists() {
            Ok(fs::read_to_string(&zimignore_path)?)
        } else {
            Ok(default_zimignore_content())
        }
    }

    pub fn set_value(&mut self, key: &str, value: &str) -> Result<(), Box<dyn Error>> {
        match key {
            "default_artist" => self.default_artist = value.to_string(),
            "normalize_project_names" => {
                self.normalize_project_names = value
                    .parse::<bool>()
                    .map_err(|_| "Value must be 'true' or 'false'")?;
            }
            _ => return Err(format!("Unknown configuration key: {key}").into()),
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    use tempfile::TempDir;

    // Use a mutex to ensure tests that modify environment variables don't run concurrently
    static ENV_MUTEX: Mutex<()> = Mutex::new(());

    #[test]
    fn test_default_artist() {
        let artist = default_artist();
        // Should either be empty or start with uppercase
        if !artist.is_empty() {
            assert!(artist.chars().next().unwrap().is_uppercase());
        }
    }

    #[test]
    fn test_default_folders() {
        let folders = default_folders();
        assert!(folders.contains(&"sources".to_string()));
        assert!(folders.contains(&"edits".to_string()));
        assert!(folders.contains(&"bounced".to_string()));
        assert!(folders.contains(&"mixes".to_string()));
        assert!(folders.contains(&"masters".to_string()));
        assert!(folders.contains(&"project".to_string()));
        assert_eq!(folders.len(), 6);
    }

    #[test]
    fn test_default_daw_folders() {
        let folders = default_daw_folders();
        assert!(folders.contains(&"live".to_string()));
        assert!(folders.contains(&"reaper".to_string()));
        assert!(folders.contains(&"bitwig".to_string()));
        assert!(folders.contains(&"renoise".to_string()));
    }

    #[test]
    fn test_default_gitignore() {
        let gitignore = default_gitignore();
        assert!(gitignore.contains(&"*.wav".to_string()));
        assert!(gitignore.contains(&"*.aif".to_string()));
        assert!(gitignore.contains(&"*.flac".to_string()));
        assert!(gitignore.contains(&"*.mp3".to_string()));
        assert!(gitignore.contains(&"*.jpg".to_string()));
        // Check that it has many entries
        assert!(gitignore.len() > 15);
    }

    #[test]
    fn test_config_new() {
        let config = Config::new();
        assert_eq!(config.default_folders, default_folders());
        assert_eq!(config.default_gitignore, default_gitignore());
        assert_eq!(config.include_readmes, true);
        assert_eq!(config.normalize_project_names, true);
    }

    #[test]
    fn test_config_default() {
        let config: Config = Default::default();
        assert_eq!(config.default_folders, default_folders());
        assert_eq!(config.default_gitignore, default_gitignore());
        assert_eq!(config.include_readmes, true);
        assert_eq!(config.normalize_project_names, true);
    }

    #[test]
    fn test_set_value() {
        let mut config = Config::new();

        // Test default_artist
        config.set_value("default_artist", "TestArtist").unwrap();
        assert_eq!(config.default_artist, "TestArtist");

        // Test normalize_project_names
        config.set_value("normalize_project_names", "true").unwrap();
        assert_eq!(config.normalize_project_names, true);

        config
            .set_value("normalize_project_names", "false")
            .unwrap();
        assert_eq!(config.normalize_project_names, false);

        // Test invalid boolean
        let result = config.set_value("normalize_project_names", "invalid");
        assert!(result.is_err());

        // Test unknown key
        let result = config.set_value("unknown_key", "value");
        assert!(result.is_err());
    }

    #[test]
    fn test_config_save_and_load() {
        let _guard = ENV_MUTEX.lock().unwrap();

        let temp_dir = TempDir::new().unwrap();
        let original_xdg = std::env::var("XDG_CONFIG_HOME").ok();
        unsafe {
            std::env::set_var("XDG_CONFIG_HOME", temp_dir.path());
        }

        // Create a unique test config
        let mut config = Config::new();
        config.default_artist = "TestArtist".to_string();
        config.save().unwrap();

        // Verify the config file was created in the temp directory
        let config_path = Config::config_path().unwrap();
        assert!(config_path.exists());

        // The path should be under temp_dir/zim/config.toml
        let expected_dir = temp_dir.path().join("zim");
        assert!(config_path.starts_with(&expected_dir));

        let loaded = Config::load().unwrap();
        assert_eq!(loaded.default_artist, "TestArtist");
        assert_eq!(loaded.default_folders, default_folders());

        // Clean up - restore original value if it existed
        unsafe {
            if let Some(original) = original_xdg {
                std::env::set_var("XDG_CONFIG_HOME", original);
            } else {
                std::env::remove_var("XDG_CONFIG_HOME");
            }
        }
    }

    #[test]
    fn test_config_exists() {
        let _guard = ENV_MUTEX.lock().unwrap();

        let temp_dir = TempDir::new().unwrap();
        let original_xdg = std::env::var("XDG_CONFIG_HOME").ok();
        unsafe {
            std::env::set_var("XDG_CONFIG_HOME", temp_dir.path());
        }

        // Verify we're checking in the temp directory
        let expected_path = temp_dir.path().join("zim").join("config.toml");
        assert!(!expected_path.exists());
        assert!(!Config::exists().unwrap());

        let config = Config::new();
        config.save().unwrap();

        assert!(expected_path.exists());
        assert!(Config::exists().unwrap());

        // Clean up - restore original value if it existed
        unsafe {
            if let Some(original) = original_xdg {
                std::env::set_var("XDG_CONFIG_HOME", original);
            } else {
                std::env::remove_var("XDG_CONFIG_HOME");
            }
        }
    }
}
