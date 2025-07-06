use crate::config::Config;
use std::error::Error;
use std::process::Command;

pub fn handle_config_view() -> Result<(), Box<dyn Error>> {
    let config = Config::load()?;

    println!("Current ZIM configuration:");
    println!("  root_dir: {}", config.root_dir);
    println!("  default_artist: {}", config.default_artist);
    println!("  default_folders: {:?}", config.default_folders);
    println!("  include_readmes: {}", config.include_readmes);

    Ok(())
}

pub fn handle_config_set(key: &str, value: &str) -> Result<(), Box<dyn Error>> {
    let mut config = Config::load()?;

    config.set_value(key, value)?;
    config.save()?;

    println!("Configuration updated: {key} = {value}");

    Ok(())
}

pub fn handle_config_edit() -> Result<(), Box<dyn Error>> {
    // Ensure config exists
    if !Config::exists()? {
        return Err("ZIM not initialized. Run 'zim init <root-dir>' first.".into());
    }

    let config_path = Config::config_path()?;
    let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vi".to_string());

    println!("Opening {} in {}", config_path.display(), editor);

    let status = Command::new(&editor)
        .arg(&config_path)
        .status()
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                format!("Editor '{editor}' not found. Set $EDITOR to a valid editor path.")
            } else {
                format!("Failed to launch editor '{editor}': {e}")
            }
        })?;

    if !status.success() {
        return Err(format!("Editor '{editor}' exited with error").into());
    }

    // Validate the config after editing
    match Config::load() {
        Ok(_) => println!("Configuration saved successfully"),
        Err(e) => {
            return Err(format!("Configuration validation failed: {e}").into());
        }
    }

    Ok(())
}
