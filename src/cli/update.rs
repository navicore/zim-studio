use crate::media::metadata::read_audio_metadata;
use crate::templates::{self, SidecarMetadata};
use std::collections::HashSet;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

pub fn handle_update(project_path: &str) -> Result<(), Box<dyn Error>> {
    let project_path = Path::new(project_path);

    // Verify this is a valid project directory
    if !project_path.exists() {
        return Err(format!("Path does not exist: {}", project_path.display()).into());
    }

    println!("Scanning project: {}", project_path.display());

    // Get audio file extensions we want sidecars for
    let audio_extensions: HashSet<&str> = ["wav", "flac", "aiff", "mp3", "m4a"]
        .iter()
        .cloned()
        .collect();

    let mut created_count = 0;
    let mut skipped_count = 0;
    let mut updated_count = 0;

    // Walk the directory tree
    scan_directory(
        project_path,
        &audio_extensions,
        &mut created_count,
        &mut skipped_count,
        &mut updated_count,
    )?;

    println!("\n✓ Update complete!");
    println!("  Created: {created_count} new sidecar files");
    println!("  Updated: {updated_count} existing files");
    println!("  Skipped: {skipped_count} files (already have sidecars)");

    Ok(())
}

fn scan_directory(
    dir: &Path,
    audio_exts: &HashSet<&str>,
    created: &mut u32,
    skipped: &mut u32,
    updated: &mut u32,
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
            scan_directory(&path, audio_exts, created, skipped, updated)?;
        } else if path.is_file() {
            // Check if this is an audio file
            if let Some(extension) = path.extension() {
                let ext = extension.to_string_lossy().to_lowercase();

                if audio_exts.contains(ext.as_str()) {
                    process_media_file(&path, created, skipped, updated)?;
                }
            }
        }
    }

    Ok(())
}

fn process_media_file(
    file_path: &Path,
    created: &mut u32,
    skipped: &mut u32,
    _updated: &mut u32,
) -> Result<(), Box<dyn Error>> {
    let sidecar_path = get_sidecar_path(file_path);

    if sidecar_path.exists() {
        *skipped += 1;
        return Ok(());
    }

    let file_name = file_path.file_name().unwrap().to_string_lossy();
    let relative_path = file_path.strip_prefix(".").unwrap_or(file_path);

    // Get file system metadata
    let file_metadata = std::fs::metadata(file_path)?;
    let file_size = file_metadata.len();

    // Get modification time
    let modified = file_metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .and_then(|duration| {
            chrono::DateTime::<chrono::Utc>::from_timestamp(duration.as_secs() as i64, 0)
        })
        .map(|dt| dt.format("%Y-%m-%d %H:%M:%S UTC").to_string());

    // Try to read audio metadata
    let extension = file_path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase());

    let content = match extension.as_deref() {
        Some("flac") | Some("wav") => {
            match read_audio_metadata(file_path) {
                Ok(metadata) => {
                    // Generate sidecar with metadata
                    templates::generate_audio_sidecar_with_metadata(&SidecarMetadata {
                        file_name: &file_name,
                        file_path: &relative_path.to_string_lossy(),
                        sample_rate: metadata.sample_rate,
                        channels: metadata.channels,
                        bits_per_sample: metadata.bits_per_sample,
                        duration_seconds: metadata.duration_seconds,
                        file_size,
                        modified: modified.as_deref(),
                    })
                }
                Err(e) => {
                    eprintln!("  Warning: Could not read metadata from {file_name}: {e}");
                    templates::generate_minimal_sidecar_with_fs_metadata(
                        &file_name,
                        &relative_path.to_string_lossy(),
                        file_size,
                        modified.as_deref(),
                    )
                }
            }
        }
        _ => {
            // Unsupported audio format - create minimal sidecar
            templates::generate_minimal_sidecar_with_fs_metadata(
                &file_name,
                &relative_path.to_string_lossy(),
                file_size,
                modified.as_deref(),
            )
        }
    };

    fs::write(&sidecar_path, content)?;
    println!("  Created: {}", sidecar_path.display());
    *created += 1;

    Ok(())
}

fn get_sidecar_path(media_path: &Path) -> PathBuf {
    let mut sidecar_path = media_path.to_path_buf();
    let current_name = sidecar_path.file_name().unwrap().to_string_lossy();
    let new_name = format!("{current_name}.md");
    sidecar_path.set_file_name(new_name);
    sidecar_path
}
