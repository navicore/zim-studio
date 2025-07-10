use crate::config::Config;
use std::error::Error;
use std::fs;
use std::path::Path;

pub fn create_project_structure(
    project_path: &Path,
    folders: &[String],
    config: &Config,
) -> Result<(), Box<dyn Error>> {
    // Create main project directory
    fs::create_dir_all(project_path)?;

    // Create subdirectories
    for folder in folders {
        let folder_path = project_path.join(folder);
        fs::create_dir_all(&folder_path)?;

        // Create README.md in each folder
        let readme_path = folder_path.join("README.md");
        let readme_content = format!(
            "# {}\n\nThis folder contains {} files.\n",
            folder,
            folder_description(folder)
        );
        fs::write(readme_path, readme_content)?;
    }

    // Create project-specific subdirectories
    let project_folder = project_path.join("project");
    for daw in &config.daw_folders {
        fs::create_dir_all(project_folder.join(daw))?;
    }

    Ok(())
}

fn folder_description(folder: &str) -> &'static str {
    match folder {
        "sources" => "raw recordings (e.g., from iPad or field mic)",
        "edits" => "chopped/trimmed versions of raw files",
        "bounced" => "Rendered/bounced tracks (stems, FX prints)",
        "mixes" => "combined track renders (pre-master)",
        "masters" => "finalized, polished versions",
        "project" => "DAW-specific session files",
        _ => "project-specific",
    }
}

pub fn create_gitignore(project_path: &Path, patterns: &[String]) -> Result<(), Box<dyn Error>> {
    let gitignore_path = project_path.join(".gitignore");
    let mut content = String::from("# Media files are backed up separately to NAS\n");
    content.push_str("# Git tracks metadata (.md files) and project structure only\n\n");
    content.push_str(&patterns.join("\n"));
    content.push('\n');
    fs::write(gitignore_path, content)?;
    Ok(())
}

pub fn create_project_metadata(
    project_path: &Path,
    project_name: &str,
    artist: &str,
    display_name: Option<&str>,
) -> Result<(), Box<dyn Error>> {
    let metadata_path = project_path.join("README.md");

    // Use display_name if provided, otherwise use project_name
    let display_title = display_name.unwrap_or(project_name);

    let content = format!(
        r#"---
name: "{}"
artist: "{}"
created: "{}"
status: "active"
tags: []
art: []
# Example art entries:
# art:
#   - path: "../shared-assets/mood-board.jpg"
#     description: "Mood board for the overall project vibe"
#     purpose: "inspiration"
#   - path: "~/Desktop/album-cover-v1.png"
#     description: "First draft of album cover"
#     purpose: "cover_art"
---

# {}

## Description

[Project description here]

## Notes

[Session notes, ideas, and documentation]

## Visual Assets

[Document any visual inspiration, artwork, or graphics associated with this project]
"#,
        display_title,
        artist,
        chrono::Local::now().format("%Y-%m-%d"),
        display_title
    );

    fs::write(metadata_path, content)?;
    Ok(())
}
