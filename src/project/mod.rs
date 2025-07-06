use std::error::Error;
use std::fs;
use std::path::Path;

pub fn create_project_structure(
    project_path: &Path,
    folders: &[String],
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
    fs::create_dir_all(project_folder.join("ableton"))?;
    fs::create_dir_all(project_folder.join("reaper"))?;

    // Create visual asset subdirectories if visuals folder exists
    if folders.iter().any(|f| f == "visuals") {
        let visuals_folder = project_path.join("visuals");
        fs::create_dir_all(visuals_folder.join("inspiration"))?;
        fs::create_dir_all(visuals_folder.join("covers"))?;
        fs::create_dir_all(visuals_folder.join("other"))?;
    }

    Ok(())
}

fn folder_description(folder: &str) -> &'static str {
    match folder {
        "sources" => "raw recordings (e.g., from iPad or field mic)",
        "edits" => "chopped/trimmed versions of raw files",
        "processed" => "EQ'd, compressed, FX-enhanced versions",
        "mixes" => "combined track renders (pre-master)",
        "masters" => "finalized, polished versions",
        "project" => "DAW-specific session files",
        "visuals" => "visual assets, artwork, and inspiration images",
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
) -> Result<(), Box<dyn Error>> {
    let metadata_path = project_path.join("README.md");
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
#   - path: "inspiration/mood-board.jpg"
#     description: "Mood board for the overall project vibe"
#     purpose: "inspiration"
#   - path: "covers/album-cover-v1.png"
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
        project_name,
        artist,
        chrono::Local::now().format("%Y-%m-%d"),
        project_name
    );

    fs::write(metadata_path, content)?;
    Ok(())
}
