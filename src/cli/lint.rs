use owo_colors::OwoColorize;
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use zim_studio::constants::{SIDECAR_EXTENSION, SKIP_DIRECTORIES, YAML_DELIMITER};
use zim_studio::utils::{progress::create_progress_spinner, validation::validate_path_exists};

// Type for duration field that can be either a number or "unknown"
#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum DurationField {
    #[allow(dead_code)]
    Number(f64),
    Unknown(String),
}

impl DurationField {
    fn validate(&self) -> Result<(), String> {
        match self {
            DurationField::Number(_) => Ok(()),
            DurationField::Unknown(s) if s == "unknown" => Ok(()),
            DurationField::Unknown(s) => {
                Err(format!("duration must be a number or 'unknown', got '{s}'"))
            }
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
enum ArtPurpose {
    Inspiration,
    CoverArt,
    Other,
}

#[derive(Debug, Deserialize, Serialize)]
struct ArtReference {
    path: String,
    #[serde(default)]
    description: String,
    purpose: ArtPurpose,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct SidecarMetadata {
    file: String,
    path: String,
    #[serde(default)]
    title: String,
    #[serde(default)]
    description: String,

    // Audio-specific fields (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    duration: Option<DurationField>,
    #[serde(skip_serializing_if = "Option::is_none")]
    sample_rate: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    channels: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    bit_depth: Option<u16>,

    // File system metadata
    file_size: u64,
    modified: String,

    // User-editable fields
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    art: Vec<ArtReference>,
}

pub fn handle_lint(project_path: &str) -> Result<(), Box<dyn Error>> {
    let project_path = Path::new(project_path);

    validate_path_exists(project_path)?;

    println!(
        "{} {}",
        "Linting project:".bright_black(),
        project_path.display().to_string().cyan()
    );
    println!();

    let spinner = create_progress_spinner();
    spinner.set_message("Scanning for sidecar files...");

    let mut total_files = 0;
    let mut valid_files = 0;
    let mut invalid_files = 0;
    let mut errors = Vec::new();

    scan_directory(
        project_path,
        &mut total_files,
        &mut valid_files,
        &mut invalid_files,
        &mut errors,
    )?;

    spinner.finish_and_clear();

    // Print results
    print_lint_results(
        project_path,
        &errors,
        total_files,
        valid_files,
        invalid_files,
    );

    if invalid_files > 0 {
        Err(format!(
            "{} Lint check failed: invalid YAML found",
            "Error:".red().bold()
        )
        .into())
    } else {
        println!(
            "\n{} {}",
            "✓".green().bold(),
            "All YAML frontmatter is valid!".green()
        );
        Ok(())
    }
}

fn scan_directory(
    dir: &Path,
    total: &mut u32,
    valid: &mut u32,
    invalid: &mut u32,
    errors: &mut Vec<(PathBuf, String)>,
) -> Result<(), Box<dyn Error>> {
    let entries = fs::read_dir(dir)?;

    for entry in entries {
        let entry = entry?;
        let path = entry.path();

        // Skip hidden files and directories
        if let Some(name) = path.file_name()
            && name.to_string_lossy().starts_with('.')
        {
            continue;
        }

        if path.is_dir() {
            // Skip certain directories
            let dir_name = path.file_name().unwrap().to_string_lossy();
            if should_skip_directory(&dir_name) {
                continue;
            }

            // Recurse into subdirectory
            scan_directory(&path, total, valid, invalid, errors)?;
        } else if path.is_file()
            && path.extension().and_then(|e| e.to_str()) == Some(SIDECAR_EXTENSION)
        {
            // Check if this is a sidecar file (has corresponding media file)
            if is_sidecar_file(&path) {
                *total += 1;
                match validate_yaml_frontmatter(&path) {
                    Ok(()) => {
                        *valid += 1;
                    }
                    Err(e) => {
                        *invalid += 1;
                        errors.push((path, e.to_string()));
                    }
                }
            }
        }
    }

    Ok(())
}

fn is_sidecar_file(md_path: &Path) -> bool {
    // A sidecar file has a name like "audio.flac.md"
    let file_name = md_path.file_name().unwrap().to_string_lossy();

    // Check if it ends with .md and has another extension before it
    if let Some(base_name) = file_name.strip_suffix(".md") {
        // Check if the base name has an extension (indicating it's a sidecar)
        Path::new(base_name).extension().is_some()
    } else {
        false
    }
}

fn validate_yaml_frontmatter(path: &Path) -> Result<(), Box<dyn Error>> {
    let content = fs::read_to_string(path)?;

    // Check if file starts with ---
    if !content.starts_with(YAML_DELIMITER) {
        return Err("Missing YAML frontmatter (file should start with ---)".into());
    }

    // Find the closing ---
    let parts: Vec<&str> = content.splitn(3, YAML_DELIMITER).collect();
    if parts.len() < 3 {
        return Err("Invalid frontmatter format (missing closing ---)".into());
    }

    // The YAML content is in parts[1]
    let yaml_content = parts[1];

    // Try to parse the YAML with schema validation
    let metadata: SidecarMetadata =
        serde_yaml::from_str(yaml_content).map_err(|e| format_validation_error(&e.to_string()))?;

    // Additional validation for duration field
    if let Some(duration) = &metadata.duration {
        duration
            .validate()
            .map_err(|e| format!("Invalid duration value: {e}"))?;
    }

    Ok(())
}

// Helper functions
fn should_skip_directory(name: &str) -> bool {
    SKIP_DIRECTORIES.contains(&name)
}

fn format_validation_error(error_msg: &str) -> String {
    // Common error patterns and helpful messages
    if error_msg.contains("missing field") {
        let field = error_msg.split('`').nth(1).unwrap_or("unknown");
        format!("Missing required field: '{field}'")
    } else if error_msg.contains("invalid type") {
        // Extract field name and type info
        if let Some(field_info) = error_msg.split("for key `").nth(1) {
            let field = field_info.split('`').next().unwrap_or("unknown");
            format!("Invalid type for field '{field}' - {error_msg}")
        } else {
            format!("Type error: {error_msg}")
        }
    } else if error_msg.contains("unknown field") {
        let field = error_msg.split('`').nth(1).unwrap_or("unknown");
        format!("Unknown field: '{field}' - check for typos in field names")
    } else if error_msg.contains("data did not match any variant") {
        "Invalid value format - check that all values have the correct type".to_string()
    } else {
        format!("Schema validation error: {error_msg}")
    }
}

fn print_lint_results(
    project_path: &Path,
    errors: &[(PathBuf, String)],
    total_files: u32,
    valid_files: u32,
    invalid_files: u32,
) {
    println!(
        "\n{} {} files scanned",
        "Summary:".bright_black(),
        total_files.to_string().cyan()
    );
    println!(
        "  {} {} valid",
        "✓".green(),
        valid_files.to_string().green()
    );
    println!(
        "  {} {} invalid",
        "✗".red(),
        invalid_files.to_string().red()
    );

    if !errors.is_empty() {
        println!("\n{}", "Errors found:".red().bold());
        for (path, error) in errors {
            let relative_path = path.strip_prefix(project_path).unwrap_or(path).display();
            println!(
                "  {} {}",
                relative_path.to_string().yellow(),
                error.bright_black()
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_duration_field_validate() {
        // Test number duration
        let duration = DurationField::Number(123.45);
        assert!(duration.validate().is_ok());

        // Test "unknown" string
        let duration = DurationField::Unknown("unknown".to_string());
        assert!(duration.validate().is_ok());

        // Test invalid string
        let duration = DurationField::Unknown("invalid".to_string());
        let result = duration.validate();
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .contains("must be a number or 'unknown'")
        );
    }

    #[test]
    fn test_is_sidecar_file() {
        // Valid sidecar files
        assert!(is_sidecar_file(Path::new("audio.flac.md")));
        assert!(is_sidecar_file(Path::new("sample.wav.md")));
        assert!(is_sidecar_file(Path::new("complex.file.name.mp3.md")));

        // Not sidecar files
        assert!(!is_sidecar_file(Path::new("README.md")));
        assert!(!is_sidecar_file(Path::new("notes.md")));
        assert!(!is_sidecar_file(Path::new("audio.txt")));
        assert!(!is_sidecar_file(Path::new(".hidden.md")));
    }

    #[test]
    fn test_should_skip_directory() {
        assert!(should_skip_directory("node_modules"));
        assert!(should_skip_directory(".git"));
        assert!(should_skip_directory("temp"));
        assert!(!should_skip_directory("src"));
        assert!(!should_skip_directory("assets"));
    }

    #[test]
    fn test_format_validation_error() {
        // Missing field error
        let error = format_validation_error("missing field `title`");
        assert_eq!(error, "Missing required field: 'title'");

        // Invalid type error with field info
        let error = format_validation_error(
            "invalid type: string \"123\", expected u32 for key `sample_rate`",
        );
        assert!(error.contains("Invalid type for field 'sample_rate'"));

        // Unknown field error
        let error = format_validation_error("unknown field `unknown_field`, expected one of");
        assert_eq!(
            error,
            "Unknown field: 'unknown_field' - check for typos in field names"
        );

        // Variant error
        let error = format_validation_error("data did not match any variant of untagged enum");
        assert_eq!(
            error,
            "Invalid value format - check that all values have the correct type"
        );

        // Generic error
        let error = format_validation_error("some other error");
        assert_eq!(error, "Schema validation error: some other error");
    }

    #[test]
    fn test_validate_yaml_frontmatter_valid() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.wav.md");

        let content = r#"---
file: test.wav
path: /test/path
title: Test Audio
description: A test audio file
duration: 123.45
sample_rate: 44100
channels: 2
bit_depth: 16
file_size: 1234567
modified: "2024-01-01T00:00:00Z"
tags: ["test", "audio"]
art: []
---

# Test Audio File

This is the content after frontmatter.
"#;

        fs::write(&file_path, content).unwrap();

        let result = validate_yaml_frontmatter(&file_path);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_yaml_frontmatter_missing() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.wav.md");

        let content = "# No frontmatter\nJust content";
        fs::write(&file_path, content).unwrap();

        let result = validate_yaml_frontmatter(&file_path);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Missing YAML frontmatter")
        );
    }

    #[test]
    fn test_validate_yaml_frontmatter_invalid_format() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.wav.md");

        let content = "---\nNo closing delimiter";
        fs::write(&file_path, content).unwrap();

        let result = validate_yaml_frontmatter(&file_path);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Invalid frontmatter format")
        );
    }

    #[test]
    fn test_validate_yaml_frontmatter_schema_error() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.wav.md");

        let content = r#"---
file: test.wav
# Missing required fields (path, file_size, modified)
---

Content"#;
        fs::write(&file_path, content).unwrap();

        let result = validate_yaml_frontmatter(&file_path);
        assert!(result.is_err());
        let error_msg = result.unwrap_err().to_string();
        // Should contain error about missing required field
        assert!(
            error_msg.contains("Missing required field"),
            "Error was: {}",
            error_msg
        );
    }

    #[test]
    fn test_validate_yaml_frontmatter_duration_unknown() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.wav.md");

        let content = r#"---
file: test.wav
path: /test/path
title: Test Audio
description: A test audio file
duration: "unknown"
file_size: 1234567
modified: "2024-01-01T00:00:00Z"
tags: []
art: []
---

Content"#;

        fs::write(&file_path, content).unwrap();

        let result = validate_yaml_frontmatter(&file_path);
        if let Err(e) = &result {
            eprintln!("Unexpected error in duration unknown test: {}", e);
        }
        assert!(result.is_ok());
    }

    #[test]
    fn test_scan_directory_integration() {
        let temp_dir = TempDir::new().unwrap();

        // Create a valid sidecar file
        let valid_file = temp_dir.path().join("audio.wav.md");
        let valid_content = r#"---
file: audio.wav
path: /test/audio.wav
title: Test Audio
description: Test
file_size: 1000
modified: "2024-01-01"
tags: []
art: []
---

Content after frontmatter"#;
        fs::write(&valid_file, valid_content).unwrap();

        // Create an invalid sidecar file
        let invalid_file = temp_dir.path().join("bad.mp3.md");
        fs::write(&invalid_file, "No frontmatter").unwrap();

        // Create a regular markdown file (should be ignored)
        let regular_md = temp_dir.path().join("README.md");
        fs::write(&regular_md, "# README").unwrap();

        // Create a subdirectory to skip
        let skip_dir = temp_dir.path().join("node_modules");
        fs::create_dir(&skip_dir).unwrap();
        let skip_file = skip_dir.join("test.wav.md");
        fs::write(&skip_file, "Should be skipped").unwrap();

        let mut total = 0;
        let mut valid = 0;
        let mut invalid = 0;
        let mut errors = Vec::new();

        let result = scan_directory(
            temp_dir.path(),
            &mut total,
            &mut valid,
            &mut invalid,
            &mut errors,
        );

        assert!(result.is_ok());

        // Debug print errors
        if !errors.is_empty() {
            for (path, err) in &errors {
                eprintln!("Error in {}: {}", path.display(), err);
            }
        }

        assert_eq!(total, 2, "Expected 2 total files, got {}", total); // Only sidecar files counted
        assert_eq!(valid, 1, "Expected 1 valid file, got {}", valid);
        assert_eq!(invalid, 1, "Expected 1 invalid file, got {}", invalid);
        assert_eq!(errors.len(), 1);
        assert!(errors[0].0.to_str().unwrap().contains("bad.mp3.md"));
    }
}
