//! Tag command for adding metadata to WAV files
//!
//! This module provides functionality to embed ZIM metadata directly into
//! WAV files using INFO LIST chunks.

use crate::wav_metadata::{self, ZimMetadata};
use owo_colors::OwoColorize;
use std::error::Error;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

pub fn handle_tag_edit(
    file: &str,
    project: Option<String>,
    no_backup: bool,
) -> Result<(), Box<dyn Error>> {
    // Validate path length (typical filesystem limit)
    const MAX_PATH_LENGTH: usize = 4096;
    if file.len() > MAX_PATH_LENGTH {
        return Err(format!(
            "{} Path too long (max {} characters)",
            "Error:".red().bold(),
            MAX_PATH_LENGTH
        )
        .into());
    }

    let path = Path::new(file);

    if !path.exists() {
        return Err(format!("{} File not found: {}", "Error:".red().bold(), file).into());
    }

    // Check if it's a WAV file
    if path.extension().and_then(|e| e.to_str()) != Some("wav") {
        return Err(format!("{} Not a WAV file: {}", "Error:".red().bold(), file).into());
    }

    println!(
        "{} {}",
        "Editing metadata in-place:".bright_black(),
        file.cyan()
    );

    // Get the absolute path for better tracking
    let abs_path = std::fs::canonicalize(path)?;

    // Check if file already has metadata
    let existing_metadata = wav_metadata::read_metadata(&abs_path)?;
    if let Some(meta) = &existing_metadata {
        println!(
            "  {} Existing UUID: {}",
            "→".bright_black(),
            meta.uuid.bright_black()
        );
    }

    // Determine project name
    let project_name = if let Some(p) = project {
        p
    } else {
        // Use the more robust project finding function
        find_project_root(&abs_path).unwrap_or_else(|| "unknown".to_string())
    };

    // Calculate MD5 of audio data
    let audio_md5 = wav_metadata::calculate_audio_md5(path)?;
    println!(
        "  {} Audio MD5: {}",
        "→".bright_black(),
        audio_md5.bright_black()
    );

    // Create or update metadata
    let metadata = if let Some(mut existing) = existing_metadata {
        // Update existing metadata while preserving UUID and lineage
        existing.project = project_name.clone();
        existing.audio_md5 = audio_md5.clone();
        // Update path in case file was moved
        existing.original_path = abs_path.to_string_lossy().to_string();
        println!("  {} Updating existing metadata", "→".bright_black());
        existing
    } else {
        // Create new metadata for untagged file
        let mut new_meta = ZimMetadata::new_original(&project_name, &abs_path);
        new_meta.audio_md5 = audio_md5.clone();
        println!("  {} Creating new metadata", "→".bright_black());
        new_meta
    };

    // Create backup if requested (default behavior)
    if !no_backup {
        // Create backup in temp dir with a unique name that preserves original filename info
        let filename = path.file_name().unwrap().to_string_lossy();
        let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
        let backup_filename = format!(
            "zim-backup-{}-{}.wav",
            filename.replace('/', "_"),
            timestamp
        );
        let backup_path = std::env::temp_dir().join(&backup_filename);
        std::fs::copy(path, &backup_path)?;
        println!(
            "  {} Backup: {}",
            "→".bright_black(),
            backup_path.display().to_string().bright_black()
        );
    }

    // Security: Create temp file in same directory with random name
    let temp_filename = format!(
        ".zim-temp-{}-{}.wav",
        std::process::id(),
        uuid::Uuid::new_v4().simple()
    );
    let temp_path = path.parent().unwrap_or(Path::new(".")).join(&temp_filename);

    // Write metadata to temp file
    let write_result = wav_metadata::write_metadata(path, &temp_path, &metadata);
    if let Err(e) = write_result {
        // Security: Clean up temp file on error
        let _ = std::fs::remove_file(&temp_path);
        return Err(e);
    }

    // Verify temp file is valid by trying to read its metadata
    match wav_metadata::read_metadata(&temp_path) {
        Ok(Some(_)) => {
            // Success - replace original with temp file
            // Security: Use atomic rename to prevent partial writes
            if let Err(e) = std::fs::rename(&temp_path, path) {
                let _ = std::fs::remove_file(&temp_path);
                return Err(format!("Failed to replace original file: {e}").into());
            }
        }
        _ => {
            // Clean up temp file
            let _ = std::fs::remove_file(&temp_path);
            return Err("Failed to write metadata correctly".into());
        }
    }

    println!("{} File updated in-place", "✓".green().bold());
    println!("  {} Project: {}", "→".bright_black(), project_name.green());
    println!("  {} UUID: {}", "→".bright_black(), metadata.uuid.green());
    println!("  {} MD5: {}", "→".bright_black(), audio_md5.bright_black());

    Ok(())
}

pub fn handle_tag(file: &str, project: Option<String>) -> Result<(), Box<dyn Error>> {
    // Validate path length
    const MAX_PATH_LENGTH: usize = 4096;
    if file.len() > MAX_PATH_LENGTH {
        return Err(format!(
            "{} Path too long (max {} characters)",
            "Error:".red().bold(),
            MAX_PATH_LENGTH
        )
        .into());
    }

    let path = Path::new(file);

    if !path.exists() {
        return Err(format!("{} File not found: {}", "Error:".red().bold(), file).into());
    }

    // Check if it's a WAV file
    if path.extension().and_then(|e| e.to_str()) != Some("wav") {
        return Err(format!("{} Not a WAV file: {}", "Error:".red().bold(), file).into());
    }

    println!("{} {}", "Tagging:".bright_black(), file.cyan());

    // Get the absolute path for better tracking
    let abs_path = std::fs::canonicalize(path)?;

    // Determine project name
    let project_name = if let Some(p) = project {
        p
    } else {
        // Use the more robust project finding function
        find_project_root(&abs_path).unwrap_or_else(|| "unknown".to_string())
    };

    // Calculate MD5 of audio data
    let audio_md5 = wav_metadata::calculate_audio_md5(path)?;
    println!(
        "  {} Audio MD5: {}",
        "→".bright_black(),
        audio_md5.bright_black()
    );

    // Create metadata with absolute path for better tracking
    let mut metadata = ZimMetadata::new_original(&project_name, &abs_path);
    metadata.audio_md5 = audio_md5.clone();

    // Create output filename (same name with _tagged suffix)
    let stem = path.file_stem().unwrap().to_string_lossy();
    let output_filename = format!("{stem}_tagged.wav");

    // Security: Validate output filename
    if output_filename.contains("..")
        || output_filename.contains('/')
        || output_filename.contains('\\')
    {
        return Err("Invalid output filename".into());
    }

    let output_path = path.with_file_name(output_filename);

    // Write metadata
    wav_metadata::write_metadata(path, &output_path, &metadata)?;

    println!(
        "{} Tagged file created: {}",
        "✓".green().bold(),
        output_path.display().to_string().yellow()
    );
    println!("  {} Project: {}", "→".bright_black(), project_name.green());
    println!("  {} MD5: {}", "→".bright_black(), audio_md5.bright_black());

    Ok(())
}

pub fn handle_tag_derive(input: &str, output: &str, transform: &str) -> Result<(), Box<dyn Error>> {
    let input_path = Path::new(input);
    let output_path = Path::new(output);

    if !input_path.exists() {
        return Err(format!("{} Input file not found: {}", "Error:".red().bold(), input).into());
    }

    // Get absolute paths for better tracking
    let abs_input_path = std::fs::canonicalize(input_path)?;
    let abs_output_path = if output_path.exists() {
        std::fs::canonicalize(output_path)?
    } else {
        // For non-existent output, resolve the parent and join the filename
        let parent = output_path.parent().unwrap_or(Path::new("."));
        let abs_parent = std::fs::canonicalize(parent)?;
        abs_parent.join(output_path.file_name().unwrap())
    };

    println!("{} Creating derived file", "Processing:".bright_black());
    println!("  {} {}", "From:".bright_black(), input.cyan());
    println!("  {} {}", "To:".bright_black(), output.cyan());
    println!("  {} {}", "Transform:".bright_black(), transform.yellow());

    // Read parent metadata if exists
    let parent_metadata = wav_metadata::read_metadata(&abs_input_path)?;

    // Create new metadata - either derived from parent or new original
    let metadata = if let Some(parent) = parent_metadata {
        let mut derived = parent.new_derived(transform);
        derived.audio_md5 = wav_metadata::calculate_audio_md5(&abs_input_path)?;
        // Update the path to the output file's absolute path
        derived.original_path = abs_output_path.to_string_lossy().to_string();
        derived
    } else {
        // No parent metadata, create new original
        let project = find_project_root(&abs_input_path).unwrap_or_else(|| "unknown".to_string());
        let mut original = ZimMetadata::new_original(&project, &abs_input_path);
        original.audio_md5 = wav_metadata::calculate_audio_md5(&abs_input_path)?;
        original.transform = Some(transform.to_string());
        original
    };

    // Write metadata to the output (write_metadata will copy the file)
    wav_metadata::write_metadata(&abs_input_path, output_path, &metadata)?;

    println!("{} Derived file created with metadata", "✓".green().bold());
    println!("  {} UUID: {}", "→".bright_black(), metadata.uuid.green());
    if let Some(parent_uuid) = &metadata.parent_uuid {
        println!("  {} Parent: {}", "→".bright_black(), parent_uuid.cyan());
    }
    println!(
        "  {} Generation: {}",
        "→".bright_black(),
        metadata.generation.to_string().yellow()
    );

    Ok(())
}

pub fn handle_tag_info(file: &str) -> Result<(), Box<dyn Error>> {
    let path = Path::new(file);

    if !path.exists() {
        return Err(format!("{} File not found: {}", "Error:".red().bold(), file).into());
    }

    println!(
        "{} {}",
        "Reading metadata from:".bright_black(),
        file.cyan()
    );

    match wav_metadata::read_metadata(path)? {
        Some(metadata) => {
            println!("\n{}", "Found ZIM metadata:".green().bold());

            println!("  {} {}", "UUID:".yellow(), metadata.uuid);

            if let Some(parent_uuid) = &metadata.parent_uuid {
                println!("  {} {}", "Parent UUID:".yellow(), parent_uuid);
            }

            println!("  {} {}", "Project:".yellow(), metadata.project);
            println!("  {} {}", "Original path:".yellow(), metadata.original_path);
            println!("  {} {}", "Generation:".yellow(), metadata.generation);

            if let Some(transform) = &metadata.transform {
                println!("  {} {}", "Transform:".yellow(), transform);
            }

            if !metadata.audio_md5.is_empty() {
                println!("  {} {}", "Audio MD5:".yellow(), metadata.audio_md5);
            }

            println!("  {} {}", "First seen:".yellow(), metadata.first_seen);
            println!("  {} {}", "Software:".yellow(), metadata.zim_version);
        }
        None => {
            println!("{} No ZIM metadata found", "!".yellow());
            println!("Use 'zim tag <file>' to add metadata");
        }
    }

    // Also calculate current audio MD5
    let current_md5 = wav_metadata::calculate_audio_md5(path)?;
    println!(
        "\n{} {}",
        "Current audio MD5:".bright_black(),
        current_md5.cyan()
    );

    Ok(())
}

/// Maximum depth to traverse when looking for project root
const MAX_PROJECT_TRAVERSAL_DEPTH: usize = 10;

/// Find the project root by looking for the nearest .zimignore file
/// This is a more robust version that handles edge cases better
fn find_project_root(file_path: &Path) -> Option<String> {
    // Start from the file's parent directory
    let mut current = file_path.parent();
    let mut depth = 0;

    while let Some(dir) = current {
        // Prevent excessive traversal
        if depth >= MAX_PROJECT_TRAVERSAL_DEPTH {
            break;
        }
        depth += 1;

        let zimignore_path = dir.join(".zimignore");
        if zimignore_path.exists() {
            // Found a project root - return its directory name
            // If this is the current working directory ("."), get the actual directory name
            if dir == Path::new(".") {
                // Get the absolute path to get the real directory name
                if let Ok(abs_path) = std::env::current_dir() {
                    return abs_path
                        .file_name()
                        .and_then(|name| name.to_str())
                        .map(|s| s.to_string());
                }
            }

            return dir
                .file_name()
                .and_then(|name| name.to_str())
                .map(|s| s.to_string());
        }
        current = dir.parent();
    }

    None
}
