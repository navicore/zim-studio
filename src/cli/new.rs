use crate::config::Config;
use crate::project;
use dialoguer::{Confirm, Input, theme::ColorfulTheme};
use indicatif::{ProgressBar, ProgressStyle};
use owo_colors::OwoColorize;
use std::error::Error;
use std::path::Path;
use std::thread;
use std::time::Duration;

pub fn handle_new(name: Option<&str>, path: Option<&str>) -> Result<(), Box<dyn Error>> {
    // Load configuration
    let config = Config::load()?;

    // Generate project name if not provided
    let project_name = match name {
        Some(n) => n.to_string(),
        None => {
            let generated = generate_project_name(path)?;
            let use_generated = Confirm::with_theme(&ColorfulTheme::default())
                .with_prompt(format!("Use auto-generated name '{}'?", generated.cyan()))
                .default(true)
                .interact()?;

            if use_generated {
                generated
            } else {
                Input::<String>::with_theme(&ColorfulTheme::default())
                    .with_prompt("Enter project name")
                    .interact_text()?
            }
        }
    };

    // Validate project name
    if project_name.is_empty() {
        return Err("Project name cannot be empty".into());
    }

    // Normalize project name if enabled
    let original_name = project_name.clone();
    let project_name = if config.normalize_project_names {
        normalize_project_name(&project_name)
    } else {
        project_name
    };

    // Create project path
    let parent_dir = match path {
        Some(p) => shellexpand::tilde(p).to_string(),
        None => std::env::current_dir()?.to_string_lossy().to_string(),
    };
    let project_path = Path::new(&parent_dir).join(&project_name);

    // Check if project already exists
    if project_path.exists() {
        eprintln!(
            "{} Project '{}' already exists at {}",
            "Error:".red().bold(),
            project_name.yellow(),
            project_path.display().to_string().cyan()
        );
        return Err("Project already exists".into());
    }

    println!(
        "{} {}",
        "Creating new project:".bright_black(),
        project_name.cyan().bold()
    );
    if config.normalize_project_names && original_name != project_name {
        println!(
            "  {} {}",
            "(normalized from:".bright_black(),
            original_name.yellow()
        );
    }
    println!(
        "{} {}",
        "Location:".bright_black(),
        project_path.display().to_string().cyan()
    );
    println!();

    let pb = ProgressBar::new(3);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.cyan} [{bar:30.cyan/blue}] {msg}")
            .unwrap()
            .progress_chars("█▓░"),
    );

    // Create project structure
    pb.set_message("Creating project structure...");
    project::create_project_structure(&project_path, &config.default_folders, &config)?;
    thread::sleep(Duration::from_millis(100));
    pb.inc(1);

    // Create .gitignore
    if !config.default_gitignore.is_empty() {
        pb.set_message("Creating .gitignore...");
        project::create_gitignore(&project_path, &config.default_gitignore)?;
        thread::sleep(Duration::from_millis(100));
    }
    pb.inc(1);

    // Create project metadata file
    pb.set_message("Creating project metadata...");
    project::create_project_metadata(&project_path, &project_name, &config.default_artist)?;
    thread::sleep(Duration::from_millis(100));
    pb.inc(1);

    pb.finish_and_clear();

    println!(
        "\n{} Project '{}' created successfully!",
        "✓".green().bold(),
        project_name.cyan().bold()
    );
    println!("\n{}", "Project structure:".yellow().bold());
    print_tree(&project_path, "", true)?;

    println!("\n{}", "Next steps:".yellow().bold());
    println!(
        "  {} {}",
        "$".bright_black(),
        format!("cd {}", project_path.display()).cyan()
    );
    println!("  {} {}", "$".bright_black(), "git init".cyan());
    println!(
        "  {} {}",
        "#".bright_black(),
        "Start creating music!".bright_black().italic()
    );

    Ok(())
}

fn generate_project_name(path: Option<&str>) -> Result<String, Box<dyn Error>> {
    let date = chrono::Local::now().format("%Y%m%d");

    // Determine parent directory
    let parent_dir = match path {
        Some(p) => shellexpand::tilde(p).to_string(),
        None => std::env::current_dir()?.to_string_lossy().to_string(),
    };

    let mut counter = 1;
    loop {
        let name = format!("{date}-{counter:03}");
        let project_path = Path::new(&parent_dir).join(&name);
        if !project_path.exists() {
            return Ok(name);
        }
        counter += 1;
    }
}

fn print_tree(dir: &Path, prefix: &str, _is_last: bool) -> Result<(), Box<dyn Error>> {
    let entries = std::fs::read_dir(dir)?;
    let mut entries: Vec<_> = entries.collect::<Result<_, _>>()?;
    entries.sort_by_key(|e| e.file_name());

    let entry_count = entries.len();

    for (index, entry) in entries.iter().enumerate() {
        let path = entry.path();
        let file_name = entry.file_name();
        let file_name_str = file_name.to_string_lossy();

        let is_last_entry = index == entry_count - 1;
        let connector = if is_last_entry {
            "└── ".bright_black().to_string()
        } else {
            "├── ".bright_black().to_string()
        };

        let styled_name = if path.is_dir() {
            file_name_str.blue().bold().to_string()
        } else if file_name_str.ends_with(".md") {
            file_name_str.green().to_string()
        } else {
            file_name_str.to_string()
        };
        println!("{prefix}{connector}{styled_name}");

        if path.is_dir() && !file_name_str.starts_with('.') {
            let extension = if is_last_entry {
                "    ".to_string()
            } else {
                "│   ".bright_black().to_string()
            };
            let new_prefix = format!("{prefix}{extension}");

            // Only recurse one level deep for cleaner output
            if prefix.is_empty() {
                print_tree(&path, &new_prefix, is_last_entry)?;
            }
        }
    }

    Ok(())
}

fn normalize_project_name(name: &str) -> String {
    // Convert to lowercase, replace spaces with underscores, remove punctuation
    name.to_lowercase()
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' {
                c
            } else if c.is_whitespace() {
                '_'
            } else {
                // Remove punctuation by replacing with nothing
                '\0'
            }
        })
        .filter(|&c| c != '\0')
        .collect::<String>()
        // Clean up multiple underscores
        .split('_')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("_")
}
