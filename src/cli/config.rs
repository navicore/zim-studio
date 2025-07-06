use crate::config::Config;
use std::error::Error;

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
