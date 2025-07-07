use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

// Type for duration field that can be either a number or "unknown"
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum DurationField {
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

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
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

    if !project_path.exists() {
        return Err(format!("Path does not exist: {}", project_path.display()).into());
    }

    println!("Linting project: {}", project_path.display());
    println!();

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

    // Print results
    if !errors.is_empty() {
        println!("❌ Found {} YAML errors:\n", errors.len());
        for (path, error) in errors {
            println!("  {}", path.display());
            println!("    Error: {error}\n");
        }
    }

    println!("Summary:");
    println!("  Total sidecar files: {total_files}");
    println!("  ✓ Valid YAML: {valid_files}");
    if invalid_files > 0 {
        println!("  ✗ Invalid YAML: {invalid_files}");
    }

    if invalid_files > 0 {
        Err("Lint check failed: invalid YAML found".into())
    } else {
        println!("\n✓ All YAML frontmatter is valid!");
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
        if let Some(name) = path.file_name() {
            if name.to_string_lossy().starts_with('.') {
                continue;
            }
        }

        if path.is_dir() {
            // Skip certain directories
            let dir_name = path.file_name().unwrap().to_string_lossy();
            if dir_name == "node_modules" || dir_name == ".git" || dir_name == "temp" {
                continue;
            }

            // Recurse into subdirectory
            scan_directory(&path, total, valid, invalid, errors)?;
        } else if path.is_file() && path.extension().and_then(|e| e.to_str()) == Some("md") {
            // Check if this is a sidecar file (has corresponding media file)
            if is_sidecar_file(&path) {
                *total += 1;
                match validate_yaml_frontmatter(&path) {
                    Ok(()) => *valid += 1,
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
    if !content.starts_with("---\n") {
        return Err("Missing YAML frontmatter (file should start with ---)".into());
    }

    // Find the closing ---
    let parts: Vec<&str> = content.splitn(3, "---\n").collect();
    if parts.len() < 3 {
        return Err("Invalid frontmatter format (missing closing ---)".into());
    }

    // The YAML content is in parts[1]
    let yaml_content = parts[1];

    // Try to parse the YAML with schema validation
    let metadata: SidecarMetadata = serde_yaml::from_str(yaml_content).map_err(|e| {
        // Provide helpful error messages based on the error type
        let error_msg = e.to_string();

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
    })?;

    // Additional validation for duration field
    if let Some(duration) = &metadata.duration {
        duration
            .validate()
            .map_err(|e| format!("Invalid duration value: {e}"))?;
    }

    Ok(())
}
