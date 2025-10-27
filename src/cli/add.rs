//! Add metadata to existing sidecar files

use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use owo_colors::OwoColorize;
use serde_yaml;
use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use zim_studio::utils::validation::validate_path_exists;

/// Handle the 'add tag' command
pub fn handle_add_tag(path: &str, tags: &[String], recursive: bool) -> Result<(), Box<dyn Error>> {
    let path = Path::new(path);

    validate_path_exists(path)?;

    let mp = MultiProgress::new();
    let spinner_style = ProgressStyle::with_template("{prefix:.bold.dim} {spinner} {wide_msg}")
        .unwrap()
        .tick_chars("⣾⣽⣻⢿⡿⣟⣯⣷");

    if path.is_file() {
        // Handle single file
        if path.extension().is_none_or(|ext| ext != "md") {
            return Err("File must be a markdown (.md) sidecar file".into());
        }

        let pb = mp.add(ProgressBar::new_spinner());
        pb.set_style(spinner_style.clone());
        pb.set_prefix("Adding tags");
        pb.enable_steady_tick(std::time::Duration::from_millis(120));

        add_tags_to_file(path, tags)?;

        pb.finish_with_message(format!(
            "✓ Added {} tag(s) to {}",
            tags.len(),
            path.display()
        ));
        println!("{} Added tags to: {}", "✓".green().bold(), path.display());
    } else {
        // Handle directory
        let pb = mp.add(ProgressBar::new_spinner());
        pb.set_style(spinner_style.clone());
        pb.set_prefix("Finding sidecar files");
        pb.enable_steady_tick(std::time::Duration::from_millis(120));

        // Collect .md files (recursively if requested)
        let mut sidecar_files = Vec::new();
        if recursive {
            collect_markdown_files_recursive(path, &mut sidecar_files)?;
        } else {
            collect_markdown_files_in_dir(path, &mut sidecar_files)?;
        }

        pb.finish_and_clear();

        if sidecar_files.is_empty() {
            return Err("No markdown sidecar files found in directory".into());
        }

        // Process all files with progress bar
        let pb = mp.add(ProgressBar::new(sidecar_files.len() as u64));
        pb.set_style(
            ProgressStyle::with_template(
                "{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} {msg}",
            )
            .unwrap()
            .progress_chars("#>-"),
        );
        pb.set_message("Adding tags to sidecar files");

        let mut success_count = 0;
        let mut error_count = 0;

        for file_path in &sidecar_files {
            pb.set_message(format!(
                "Processing {}",
                file_path.file_name().unwrap_or_default().to_string_lossy()
            ));

            match add_tags_to_file(file_path, tags) {
                Ok(_) => success_count += 1,
                Err(e) => {
                    eprintln!(
                        "{} Failed to update {}: {}",
                        "✗".red(),
                        file_path.display(),
                        e
                    );
                    error_count += 1;
                }
            }

            pb.inc(1);
        }

        pb.finish_with_message("Done");

        println!(
            "{} Added tags to {} file(s) ({} errors)",
            "✓".green().bold(),
            success_count,
            error_count
        );
    }

    Ok(())
}

/// Collect markdown files only in the specified directory (not recursive)
fn collect_markdown_files_in_dir(
    dir: &Path,
    files: &mut Vec<PathBuf>,
) -> Result<(), Box<dyn Error>> {
    let entries = fs::read_dir(dir)?;

    for entry in entries {
        let entry = entry?;
        let path = entry.path();

        if path.is_file() && path.extension().is_some_and(|ext| ext == "md") {
            files.push(path);
        }
    }

    Ok(())
}

/// Recursively collect all markdown files in a directory and subdirectories
fn collect_markdown_files_recursive(
    dir: &Path,
    files: &mut Vec<PathBuf>,
) -> Result<(), Box<dyn Error>> {
    let entries = fs::read_dir(dir)?;

    for entry in entries {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir() {
            // Skip hidden directories
            if path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with('.'))
            {
                continue;
            }
            // Recursively search subdirectories
            collect_markdown_files_recursive(&path, files)?;
        } else if path.extension().is_some_and(|ext| ext == "md") {
            files.push(path);
        }
    }

    Ok(())
}

/// Add tags to a single sidecar file
fn add_tags_to_file(path: &Path, new_tags: &[String]) -> Result<(), Box<dyn Error>> {
    let content = fs::read_to_string(path)?;

    // Check if file has YAML frontmatter
    if !content.starts_with("---\n") {
        return Err("File does not have YAML frontmatter".into());
    }

    // Find the end of the frontmatter
    let frontmatter_end_pos = content[4..]
        .find("---\n")
        .ok_or("Invalid YAML frontmatter: no closing delimiter")?
        + 4;

    let yaml_section = &content[4..frontmatter_end_pos];
    let markdown_section = &content[frontmatter_end_pos + 4..];

    // Parse YAML
    let mut yaml_data: HashMap<String, serde_yaml::Value> = serde_yaml::from_str(yaml_section)?;

    // Get existing tags or create empty set
    let existing_tags: HashSet<String> = if let Some(tags_value) = yaml_data.get("tags") {
        if let serde_yaml::Value::Sequence(tags_seq) = tags_value {
            tags_seq
                .iter()
                .filter_map(|v| {
                    if let serde_yaml::Value::String(s) = v {
                        Some(s.clone())
                    } else {
                        None
                    }
                })
                .collect()
        } else if let serde_yaml::Value::String(single_tag) = tags_value {
            let mut set = HashSet::new();
            set.insert(single_tag.clone());
            set
        } else {
            HashSet::new()
        }
    } else {
        HashSet::new()
    };

    // Combine existing and new tags
    let mut all_tags: HashSet<String> = existing_tags;
    for tag in new_tags {
        all_tags.insert(tag.clone());
    }

    // Convert back to sorted list for consistency
    let mut tags_list: Vec<String> = all_tags.into_iter().collect();
    tags_list.sort();

    // Update YAML data
    yaml_data.insert(
        "tags".to_string(),
        serde_yaml::Value::Sequence(
            tags_list
                .into_iter()
                .map(serde_yaml::Value::String)
                .collect(),
        ),
    );

    // Serialize back to YAML string
    let updated_yaml = serde_yaml::to_string(&yaml_data)?;

    // Reconstruct the content
    let updated_content = format!("---\n{updated_yaml}---\n{markdown_section}");

    // Write back to file
    fs::write(path, updated_content)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_add_tags_to_new_file() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.md");

        // Create a test file with YAML frontmatter but no tags
        let content = r#"---
title: Test File
artist: Test Artist
---
# Test Content

This is test content.
"#;
        fs::write(&file_path, content).unwrap();

        // Add tags
        let tags = vec!["tag1".to_string(), "tag2".to_string()];
        add_tags_to_file(&file_path, &tags).unwrap();

        // Read and verify
        let result = fs::read_to_string(&file_path).unwrap();
        assert!(result.contains("tags:"));
        assert!(result.contains("- tag1"));
        assert!(result.contains("- tag2"));
        assert!(result.contains("# Test Content"));
    }

    #[test]
    fn test_add_tags_to_existing_tags() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.md");

        // Create a test file with existing tags
        let content = r#"---
title: Test File
tags:
  - existing1
  - existing2
---
# Test Content
"#;
        fs::write(&file_path, content).unwrap();

        // Add new tags
        let tags = vec!["new1".to_string(), "existing1".to_string()]; // One duplicate
        add_tags_to_file(&file_path, &tags).unwrap();

        // Read and verify
        let result = fs::read_to_string(&file_path).unwrap();
        assert!(result.contains("- existing1"));
        assert!(result.contains("- existing2"));
        assert!(result.contains("- new1"));
        // Should not have duplicate existing1
        let existing1_count = result.matches("- existing1").count();
        assert_eq!(existing1_count, 1);
    }
}
