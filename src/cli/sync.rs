use crate::media::metadata::read_audio_metadata;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use owo_colors::OwoColorize;
use std::collections::HashSet;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use zim_studio::zimignore::ZimIgnore;

// Constants
const AUDIO_EXTENSIONS: &[&str] = &["wav", "flac", "aiff", "mp3", "m4a"];
const SKIP_DIRECTORIES: &[&str] = &["node_modules", ".git", "temp"];

pub fn handle_sync(project_path: &str) -> Result<(), Box<dyn Error>> {
    let project_path = Path::new(project_path);

    // Verify this is a valid project directory
    if !project_path.exists() {
        return Err(format!(
            "{} Path does not exist: {}",
            "Error:".red().bold(),
            project_path.display()
        )
        .into());
    }

    println!(
        "{} {}",
        "Syncing metadata in:".bright_black(),
        project_path.display().to_string().cyan()
    );

    let audio_extensions: HashSet<&str> = AUDIO_EXTENSIONS.iter().cloned().collect();
    let zimignore = ZimIgnore::load_for_directory(project_path);

    // Count files that need syncing
    let spinner = create_progress_spinner();
    spinner.set_message("Scanning for audio files with sidecars...");

    let files_to_sync = find_files_to_sync(project_path, &audio_extensions, &zimignore)?;
    spinner.finish_and_clear();

    if files_to_sync.is_empty() {
        println!("{} No audio files with sidecars found", "⚠".yellow());
        return Ok(());
    }

    println!(
        "{} Found {} files to check for sync\n",
        "ℹ".blue(),
        files_to_sync.len().to_string().cyan().bold()
    );

    let synced_count = Arc::new(Mutex::new(0));
    let skipped_count = Arc::new(Mutex::new(0));
    let error_count = Arc::new(Mutex::new(0));

    let multi = MultiProgress::new();
    let pb = multi.add(create_progress_bar(files_to_sync.len() as u64));
    pb.set_message("Syncing metadata...");

    for (audio_path, sidecar_path) in &files_to_sync {
        let file_name = audio_path.file_name().unwrap().to_string_lossy();
        pb.set_message(format!("Checking: {file_name}"));

        match sync_sidecar_metadata(audio_path, sidecar_path) {
            Ok(true) => {
                *synced_count.lock().unwrap() += 1;
                pb.set_message(format!("Synced: {}", file_name.green()));
            }
            Ok(false) => {
                *skipped_count.lock().unwrap() += 1;
                pb.set_message(format!("Up to date: {}", file_name.bright_black()));
            }
            Err(e) => {
                *error_count.lock().unwrap() += 1;
                eprintln!(
                    "  {} Failed to sync {}: {}",
                    "Error:".red(),
                    file_name.red(),
                    e.to_string().bright_black()
                );
            }
        }
        pb.inc(1);
    }

    pb.finish_with_message("Done");

    let synced = *synced_count.lock().unwrap();
    let skipped = *skipped_count.lock().unwrap();
    let errors = *error_count.lock().unwrap();

    print_sync_summary(synced, skipped, errors);

    Ok(())
}

fn find_files_to_sync(
    dir: &Path,
    audio_exts: &HashSet<&str>,
    zimignore: &ZimIgnore,
) -> Result<Vec<(PathBuf, PathBuf)>, Box<dyn Error>> {
    let mut files_to_sync = Vec::new();
    scan_for_sync(dir, audio_exts, zimignore, &mut files_to_sync)?;
    Ok(files_to_sync)
}

fn scan_for_sync(
    dir: &Path,
    audio_exts: &HashSet<&str>,
    zimignore: &ZimIgnore,
    files_to_sync: &mut Vec<(PathBuf, PathBuf)>,
) -> Result<(), Box<dyn Error>> {
    let entries = fs::read_dir(dir)?;

    for entry in entries {
        let entry = entry?;
        let path = entry.path();

        // Skip hidden files and directories
        if is_hidden_file(&path) {
            continue;
        }

        // Check if this path should be ignored by .zimignore
        if zimignore.is_ignored(&path, path.is_dir()) {
            continue;
        }

        if path.is_dir() {
            // Skip certain directories
            let dir_name = path.file_name().unwrap().to_string_lossy();
            if should_skip_directory(&dir_name) {
                continue;
            }

            // Recurse into subdirectory
            scan_for_sync(&path, audio_exts, zimignore, files_to_sync)?;
        } else if path.is_file() {
            // Check if this is an audio file
            if let Some(extension) = path.extension() {
                let ext = extension.to_string_lossy().to_lowercase();

                if audio_exts.contains(ext.as_str()) {
                    // Check if sidecar exists
                    let sidecar_path = get_sidecar_path(&path);
                    if sidecar_path.exists() {
                        files_to_sync.push((path.clone(), sidecar_path));
                    }
                }
            }
        }
    }

    Ok(())
}

fn sync_sidecar_metadata(audio_path: &Path, sidecar_path: &Path) -> Result<bool, Box<dyn Error>> {
    // Read current sidecar content
    let sidecar_content = fs::read_to_string(sidecar_path)?;

    // Extract current audio metadata
    let file_metadata = fs::metadata(audio_path)?;
    let file_size = file_metadata.len();
    let modified = file_metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .and_then(|duration| {
            chrono::DateTime::<chrono::Utc>::from_timestamp(duration.as_secs() as i64, 0)
        })
        .map(|dt| dt.format("%Y-%m-%d %H:%M:%S UTC").to_string());

    // Try to read detailed audio metadata
    let audio_metadata = read_audio_metadata(audio_path).ok();

    // Parse and update the YAML frontmatter
    let updated_content = update_yaml_fields(
        &sidecar_content,
        file_size,
        modified.as_deref(),
        audio_metadata.as_ref(),
    )?;

    // Check if content changed
    if updated_content != sidecar_content {
        fs::write(sidecar_path, updated_content)?;
        Ok(true) // File was synced
    } else {
        Ok(false) // File was already up to date
    }
}

fn update_yaml_fields(
    content: &str,
    file_size: u64,
    modified: Option<&str>,
    audio_metadata: Option<&crate::media::metadata::AudioMetadata>,
) -> Result<String, Box<dyn Error>> {
    // Check if content has YAML frontmatter
    if !content.starts_with("---\n") {
        return Ok(content.to_string()); // No frontmatter, return as-is
    }

    // Find the end of frontmatter
    let frontmatter_end = content[4..].find("\n---\n").map(|pos| pos + 4 + 5); // +4 for "---\n" at start, +5 for "\n---\n"

    if frontmatter_end.is_none() {
        return Ok(content.to_string()); // Invalid frontmatter, return as-is
    }

    let frontmatter_end = frontmatter_end.unwrap();
    let yaml_section = &content[4..frontmatter_end - 5]; // Extract YAML content
    let markdown_section = &content[frontmatter_end..];

    // Parse YAML line by line and update specific fields
    let mut updated_lines = Vec::new();
    let mut seen_fields = HashSet::new();

    for line in yaml_section.lines() {
        let trimmed = line.trim_start();

        // Check if this is a field we want to update
        if let Some((key, _)) = parse_yaml_field(trimmed) {
            match key {
                "duration" => {
                    seen_fields.insert("duration");
                    if let Some(meta) = audio_metadata {
                        if let Some(duration) = meta.duration_seconds {
                            updated_lines.push(format!("duration: {duration:.2}"));
                        } else {
                            updated_lines.push("duration: unknown".to_string());
                        }
                    } else {
                        updated_lines.push(line.to_string());
                    }
                }
                "sample_rate" => {
                    seen_fields.insert("sample_rate");
                    if let Some(meta) = audio_metadata {
                        updated_lines.push(format!("sample_rate: {}", meta.sample_rate));
                    } else {
                        updated_lines.push(line.to_string());
                    }
                }
                "channels" => {
                    seen_fields.insert("channels");
                    if let Some(meta) = audio_metadata {
                        updated_lines.push(format!("channels: {}", meta.channels));
                    } else {
                        updated_lines.push(line.to_string());
                    }
                }
                "bit_depth" => {
                    seen_fields.insert("bit_depth");
                    if let Some(meta) = audio_metadata {
                        updated_lines.push(format!("bit_depth: {}", meta.bits_per_sample));
                    } else {
                        updated_lines.push(line.to_string());
                    }
                }
                "file_size" => {
                    seen_fields.insert("file_size");
                    updated_lines.push(format!("file_size: {file_size}"));
                }
                "modified" => {
                    seen_fields.insert("modified");
                    let mod_str = modified.unwrap_or("unknown");
                    updated_lines.push(format!("modified: \"{mod_str}\""));
                }
                _ => {
                    // Keep other fields as-is
                    updated_lines.push(line.to_string());
                }
            }
        } else {
            // Not a field line, keep as-is
            updated_lines.push(line.to_string());
        }
    }

    // Add any missing fields (in case they weren't in the original)
    if !seen_fields.contains("duration")
        && let Some(meta) = audio_metadata
        && let Some(duration) = meta.duration_seconds
    {
        updated_lines.push(format!("duration: {duration:.2}"));
    }
    if !seen_fields.contains("sample_rate")
        && let Some(meta) = audio_metadata
    {
        updated_lines.push(format!("sample_rate: {}", meta.sample_rate));
    }
    if !seen_fields.contains("channels")
        && let Some(meta) = audio_metadata
    {
        updated_lines.push(format!("channels: {}", meta.channels));
    }
    if !seen_fields.contains("bit_depth")
        && let Some(meta) = audio_metadata
    {
        updated_lines.push(format!("bit_depth: {}", meta.bits_per_sample));
    }
    if !seen_fields.contains("file_size") {
        updated_lines.push(format!("file_size: {file_size}"));
    }
    if !seen_fields.contains("modified") {
        let mod_str = modified.unwrap_or("unknown");
        updated_lines.push(format!("modified: \"{mod_str}\""));
    }

    // Reconstruct the content
    let updated_yaml = updated_lines.join("\n");
    Ok(format!("---\n{updated_yaml}\n---\n{markdown_section}"))
}

fn parse_yaml_field(line: &str) -> Option<(&str, &str)> {
    if let Some(colon_pos) = line.find(':') {
        let key = line[..colon_pos].trim();
        let value = line[colon_pos + 1..].trim();
        Some((key, value))
    } else {
        None
    }
}

fn get_sidecar_path(media_path: &Path) -> PathBuf {
    let mut sidecar_path = media_path.to_path_buf();
    let current_name = sidecar_path.file_name().unwrap().to_string_lossy();
    let new_name = format!("{current_name}.md");
    sidecar_path.set_file_name(new_name);
    sidecar_path
}

fn should_skip_directory(name: &str) -> bool {
    SKIP_DIRECTORIES.contains(&name)
}

fn is_hidden_file(path: &Path) -> bool {
    path.file_name()
        .map(|name| name.to_string_lossy().starts_with('.'))
        .unwrap_or(false)
}

fn create_progress_spinner() -> ProgressBar {
    let spinner = ProgressBar::new_spinner();
    spinner.set_style(
        ProgressStyle::default_spinner()
            .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏")
            .template("{spinner:.cyan} {msg}")
            .unwrap(),
    );
    spinner.enable_steady_tick(std::time::Duration::from_millis(80));
    spinner
}

fn create_progress_bar(total: u64) -> ProgressBar {
    let pb = ProgressBar::new(total);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} {msg}")
            .unwrap()
            .progress_chars("#>-"),
    );
    pb
}

fn print_sync_summary(synced: u32, skipped: u32, errors: u32) {
    println!("\n{} {}", "✓".green().bold(), "Sync complete!".bold());
    println!(
        "  {} {} files updated",
        "Synced:".bright_black(),
        synced.to_string().green().bold()
    );
    println!(
        "  {} {} files already up to date",
        "Skipped:".bright_black(),
        skipped.to_string().yellow().bold()
    );
    if errors > 0 {
        println!(
            "  {} {} files had errors",
            "Errors:".bright_black(),
            errors.to_string().red().bold()
        );
    }
}
