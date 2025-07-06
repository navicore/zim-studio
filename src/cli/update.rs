use crate::templates;
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

    // Get all media file extensions we care about
    let audio_extensions: HashSet<&str> = ["wav", "flac", "aiff", "mp3", "m4a"]
        .iter()
        .cloned()
        .collect();
    let visual_extensions: HashSet<&str> = [
        "jpg", "jpeg", "png", "gif", "mp4", "mov", "avi", "webm", "tiff", "bmp", "heic", "heif",
    ]
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
        &visual_extensions,
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
    visual_exts: &HashSet<&str>,
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
            scan_directory(&path, audio_exts, visual_exts, created, skipped, updated)?;
        } else if path.is_file() {
            // Check if this is a media file
            if let Some(extension) = path.extension() {
                let ext = extension.to_string_lossy().to_lowercase();

                if audio_exts.contains(ext.as_str()) || visual_exts.contains(ext.as_str()) {
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

    // Create minimal sidecar for now
    let file_name = file_path.file_name().unwrap().to_string_lossy();
    let relative_path = file_path.strip_prefix(".").unwrap_or(file_path);

    let content = templates::generate_minimal_sidecar(&file_name, &relative_path.to_string_lossy());

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
