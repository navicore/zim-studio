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
        _ => "project-specific",
    }
}

pub fn create_gitignore(project_path: &Path, patterns: &[String]) -> Result<(), Box<dyn Error>> {
    let gitignore_path = project_path.join(".gitignore");
    let content = patterns.join("\n");
    fs::write(gitignore_path, content)?;
    Ok(())
}

pub fn create_project_metadata(
    project_path: &Path,
    project_name: &str,
    artist: &str,
) -> Result<(), Box<dyn Error>> {
    let metadata_path = project_path.join(format!("{project_name}.md"));
    let content = format!(
        r#"---
name: "{}"
artist: "{}"
created: "{}"
status: "active"
tags: []
---

# {}

## Description

[Project description here]

## Notes

[Session notes, ideas, and documentation]
"#,
        project_name,
        artist,
        chrono::Local::now().format("%Y-%m-%d"),
        project_name
    );

    fs::write(metadata_path, content)?;
    Ok(())
}
