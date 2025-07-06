use crate::config::Config;
use std::error::Error;
use std::fs;
use std::path::Path;

pub fn handle_init(root_dir: &str) -> Result<(), Box<dyn Error>> {
    // Check if already initialized
    if Config::exists()? {
        return Err("ZIM is already initialized. Use 'zim config set root_dir <path>' to change the root directory.".into());
    }

    // Expand tilde if present
    let expanded_path = shellexpand::tilde(root_dir);
    let root_path = Path::new(expanded_path.as_ref());

    // Create directory if it doesn't exist
    if !root_path.exists() {
        println!("Creating root directory: {}", root_path.display());
        fs::create_dir_all(root_path)?;
    } else if !root_path.is_dir() {
        return Err(format!("{} exists but is not a directory", root_path.display()).into());
    }

    // Create and save config
    let config = Config::new(root_path.to_string_lossy().to_string());
    config.save()?;

    println!("ZIM initialized successfully!");
    println!("Root directory: {}", root_path.display());
    println!(
        "Configuration saved to: {}",
        Config::config_path()?.display()
    );

    Ok(())
}
