use crate::config::Config;
use indicatif::{ProgressBar, ProgressStyle};
use owo_colors::OwoColorize;
use std::error::Error;
use std::fs;
use std::path::Path;
use std::thread;
use std::time::Duration;

pub fn handle_init(root_dir: &str) -> Result<(), Box<dyn Error>> {
    // Check if already initialized
    if Config::exists()? {
        return Err(format!(
            "{} ZIM is already initialized. Use {} to change the root directory.",
            "Error:".red().bold(),
            "'zim config set root_dir <path>'".cyan()
        )
        .into());
    }

    // Expand tilde if present
    let expanded_path = shellexpand::tilde(root_dir);
    let root_path = Path::new(expanded_path.as_ref());

    // Create directory if it doesn't exist
    if !root_path.exists() {
        let spinner = ProgressBar::new_spinner();
        spinner.set_style(
            ProgressStyle::default_spinner()
                .template("{spinner:.cyan} {msg}")
                .unwrap()
                .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"]),
        );
        spinner.set_message(format!(
            "Creating root directory: {}",
            root_path.display().to_string().bright_blue()
        ));

        fs::create_dir_all(root_path)?;
        thread::sleep(Duration::from_millis(200)); // Brief pause for visual effect

        spinner.finish_with_message(format!("{} Created root directory", "✓".green().bold()));
    } else if !root_path.is_dir() {
        return Err(format!(
            "{} {} exists but is not a directory",
            "Error:".red().bold(),
            root_path.display()
        )
        .into());
    }

    // Create and save config
    let spinner = ProgressBar::new_spinner();
    spinner.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.cyan} {msg}")
            .unwrap()
            .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"]),
    );
    spinner.set_message("Saving configuration...");

    let config = Config::new(root_path.to_string_lossy().to_string());
    config.save()?;

    thread::sleep(Duration::from_millis(200));
    spinner.finish_and_clear();

    println!(
        "{} {}",
        "✓".green().bold(),
        "ZIM initialized successfully!".bold()
    );
    println!(
        "  {} {}",
        "Root directory:".bright_black(),
        root_path.display().to_string().cyan()
    );
    println!(
        "  {} {}",
        "Configuration:".bright_black(),
        Config::config_path()?.display().to_string().cyan()
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
