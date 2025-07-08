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
    pub root_dir: String,
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

impl Config {
    pub fn new(root_dir: String) -> Self {
        Self {
            root_dir,
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

    pub fn load() -> Result<Self, Box<dyn Error>> {
        let config_path = Self::config_path()?;

        if !config_path.exists() {
            return Err("ZIM not initialized. Run 'zim init <root-dir>' first.".into());
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

    pub fn set_value(&mut self, key: &str, value: &str) -> Result<(), Box<dyn Error>> {
        match key {
            "root_dir" => self.root_dir = value.to_string(),
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
