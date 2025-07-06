use crate::config::Config;
use crate::project;
use std::error::Error;
use std::path::Path;

pub fn handle_new(name: Option<&str>) -> Result<(), Box<dyn Error>> {
    // Load configuration
    let config = Config::load()?;

    // Generate project name if not provided
    let project_name = match name {
        Some(n) => n.to_string(),
        None => generate_project_name()?,
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
    let root_dir = shellexpand::tilde(&config.root_dir);
    let project_path = Path::new(root_dir.as_ref()).join(&project_name);

    // Check if project already exists
    if project_path.exists() {
        return Err(format!(
            "Project '{}' already exists at {}",
            project_name,
            project_path.display()
        )
        .into());
    }

    println!("Creating new project: {project_name}");
    if config.normalize_project_names && original_name != project_name {
        println!("  (normalized from: {original_name})");
    }
    println!("Location: {}", project_path.display());

    // Create project structure
    project::create_project_structure(&project_path, &config.default_folders)?;

    // Create .gitignore
    if !config.default_gitignore.is_empty() {
        project::create_gitignore(&project_path, &config.default_gitignore)?;
    }

    // Create project metadata file
    project::create_project_metadata(&project_path, &project_name, &config.default_artist)?;

    println!("\n✓ Project '{project_name}' created successfully!");
    println!("\nProject structure:");
    print_tree(&project_path, "", true)?;

    println!("\nNext steps:");
    println!("  cd {}", project_path.display());
    println!("  git init");
    println!("  # Start creating music!");

    Ok(())
}

fn generate_project_name() -> Result<String, Box<dyn Error>> {
    let date = chrono::Local::now().format("%Y%m%d");

    // Find the next available number for today
    let counter = 1;
    let name = format!("{date}-{counter:03}");
    // We'll check if it exists when we create the full path
    // For now, just return the first attempt
    Ok(name)
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
            "└── "
        } else {
            "├── "
        };

        println!("{prefix}{connector}{file_name_str}");

        if path.is_dir() && !file_name_str.starts_with('.') {
            let extension = if is_last_entry { "    " } else { "│   " };
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
