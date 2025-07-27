use crate::config::Config;
use indicatif::{ProgressBar, ProgressStyle};
use owo_colors::OwoColorize;
use std::error::Error;
use std::thread;
use std::time::Duration;

pub fn handle_init() -> Result<(), Box<dyn Error>> {
    // Check if already initialized
    if Config::exists()? {
        eprintln!(
            "{} ZIM is already initialized. Use {} to edit configuration.",
            "Error:".red().bold(),
            "'zim config edit'".cyan()
        );
        return Err("Configuration already exists".into());
    }

    // Create and save default config
    let spinner = ProgressBar::new_spinner();
    spinner.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.cyan} {msg}")
            .unwrap()
            .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"]),
    );
    spinner.set_message("Creating default configuration...");

    let config: Config = Default::default();
    config.save()?;

    // Also create the default .zimignore template
    Config::ensure_default_zimignore()?;

    thread::sleep(Duration::from_millis(200));
    spinner.finish_and_clear();

    println!(
        "{} {}",
        "✓".green().bold(),
        "ZIM initialized successfully!".bold()
    );
    println!(
        "  {} {}",
        "Configuration:".bright_black(),
        Config::config_path()?.display().to_string().cyan()
    );
    println!(
        "  {} {}",
        "Default .zimignore:".bright_black(),
        Config::default_zimignore_path()?
            .display()
            .to_string()
            .cyan()
    );
    println!();
    println!("{}", "Default configuration created:".yellow().bold());
    println!("  • Projects will be created relative to current directory");
    println!(
        "  • Use {} to set a default root directory",
        "'zim config set root_dir <path>'".cyan()
    );
    println!(
        "  • Use {} to customize other settings",
        "'zim config edit'".cyan()
    );
    println!();
    println!("{}", "Next steps:".yellow().bold());
    println!(
        "  {} to create your first project",
        "zim new <project-name>".cyan()
    );
    println!("  {} to view configuration", "zim config view".cyan());

    Ok(())
}
