use tempfile::TempDir;

#[test]
fn test_config_lifecycle() {
    // Create a temporary directory for test config
    let temp_dir = TempDir::new().unwrap();

    // Override the config path for testing
    unsafe {
        std::env::set_var("XDG_CONFIG_HOME", temp_dir.path());
    }

    // Test that config doesn't exist initially
    assert!(!zim_studio::config::Config::exists().unwrap());

    // Create and save a config
    let config = zim_studio::config::Config::new();
    config.save().unwrap();

    // Verify it exists now
    assert!(zim_studio::config::Config::exists().unwrap());

    // Load and verify values
    let loaded = zim_studio::config::Config::load().unwrap();
    assert_eq!(loaded.default_folders.len(), 6);
    assert!(loaded.default_folders.contains(&"sources".to_string()));
    assert!(loaded.default_folders.contains(&"masters".to_string()));

    // Test config mutation
    let mut config = zim_studio::config::Config::load().unwrap();
    config.set_value("default_artist", "test_artist").unwrap();
    config.save().unwrap();

    // Verify mutations persisted
    let reloaded = zim_studio::config::Config::load().unwrap();
    assert_eq!(reloaded.default_artist, "test_artist");

    // Test invalid key
    let mut config = zim_studio::config::Config::load().unwrap();
    assert!(config.set_value("invalid_key", "value").is_err());
}
