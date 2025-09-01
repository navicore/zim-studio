use crate::media::metadata::read_audio_metadata;
use crate::templates::{self, SidecarMetadata};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use owo_colors::OwoColorize;
use serde_yaml;
use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::SystemTime;
use zim_studio::zimignore::ZimIgnore;

// Constants
const AUDIO_EXTENSIONS: &[&str] = &["wav", "flac", "aiff", "mp3", "m4a"];
const SKIP_DIRECTORIES: &[&str] = &["node_modules", ".git", "temp"];
const SPINNER_CHARS: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
const SIDECAR_EXTENSION: &str = "md";

pub fn handle_update(project_path: &str) -> Result<(), Box<dyn Error>> {
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
        "Scanning project:".bright_black(),
        project_path.display().to_string().cyan()
    );

    // Get audio file extensions we want sidecars for
    let audio_extensions: HashSet<&str> = AUDIO_EXTENSIONS.iter().cloned().collect();

    // Load .zimignore files for this directory hierarchy
    let zimignore = ZimIgnore::load_for_directory(project_path);

    // First, count total audio files
    let spinner = create_progress_spinner();
    spinner.set_message("Counting audio files...");

    let total_files = count_audio_files(project_path, &audio_extensions, &zimignore)?;
    spinner.finish_and_clear();

    if total_files == 0 {
        println!("{} No audio files found in project", "⚠".yellow());
        return Ok(());
    }

    println!(
        "{} Found {} audio files\n",
        "ℹ".blue(),
        total_files.to_string().cyan().bold()
    );

    let created_count = Arc::new(Mutex::new(0));
    let skipped_count = Arc::new(Mutex::new(0));
    let updated_count = Arc::new(Mutex::new(0));

    let multi = MultiProgress::new();
    let pb = multi.add(create_progress_bar(total_files as u64));
    pb.set_message("Processing audio files...");

    // Walk the directory tree
    scan_directory(
        project_path,
        &audio_extensions,
        &zimignore,
        &created_count,
        &skipped_count,
        &updated_count,
        &pb,
    )?;

    pb.finish_with_message("Done");

    let created = *created_count.lock().unwrap();
    let updated = *updated_count.lock().unwrap();
    let skipped = *skipped_count.lock().unwrap();

    print_update_summary(created, updated, skipped);

    Ok(())
}

fn count_audio_files(
    dir: &Path,
    audio_exts: &HashSet<&str>,
    zimignore: &ZimIgnore,
) -> Result<u32, Box<dyn Error>> {
    let mut count = 0;
    let entries = fs::read_dir(dir)?;

    for entry in entries {
        let entry = entry?;
        let path = entry.path();

        if is_hidden_file(&path) {
            continue;
        }

        // Check if this path should be ignored by .zimignore
        if zimignore.is_ignored(&path, path.is_dir()) {
            continue;
        }

        if path.is_dir() {
            let dir_name = path.file_name().unwrap().to_string_lossy();
            if should_skip_directory(&dir_name) {
                continue;
            }
            count += count_audio_files(&path, audio_exts, zimignore)?;
        } else if path.is_file()
            && let Some(extension) = path.extension()
        {
            let ext = extension.to_string_lossy().to_lowercase();
            if audio_exts.contains(ext.as_str()) {
                count += 1;
            }
        }
    }

    Ok(count)
}

fn scan_directory(
    dir: &Path,
    audio_exts: &HashSet<&str>,
    zimignore: &ZimIgnore,
    created: &Arc<Mutex<u32>>,
    skipped: &Arc<Mutex<u32>>,
    updated: &Arc<Mutex<u32>>,
    pb: &ProgressBar,
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
            scan_directory(&path, audio_exts, zimignore, created, skipped, updated, pb)?;
        } else if path.is_file() {
            // Check if this is an audio file
            if let Some(extension) = path.extension() {
                let ext = extension.to_string_lossy().to_lowercase();

                if audio_exts.contains(ext.as_str()) {
                    process_media_file(&path, created, skipped, updated, pb)?;
                    pb.inc(1);
                }
            }
        }
    }

    Ok(())
}

fn process_media_file(
    file_path: &Path,
    created: &Arc<Mutex<u32>>,
    skipped: &Arc<Mutex<u32>>,
    updated: &Arc<Mutex<u32>>,
    pb: &ProgressBar,
) -> Result<(), Box<dyn Error>> {
    let sidecar_path = get_sidecar_path(file_path);

    let file_name = file_path.file_name().unwrap().to_string_lossy();

    if sidecar_path.exists() {
        // Check if audio file is newer than sidecar
        let audio_metadata = fs::metadata(file_path)?;
        let sidecar_metadata = fs::metadata(&sidecar_path)?;

        if let (Ok(audio_time), Ok(sidecar_time)) =
            (audio_metadata.modified(), sidecar_metadata.modified())
            && audio_time > sidecar_time
        {
            // Audio file is newer - offer to update
            pb.suspend(|| -> Result<(), Box<dyn Error>> {
                if offer_metadata_update(file_path, &sidecar_path, updated)? {
                    Ok(())
                } else {
                    // User declined - touch the sidecar to update its timestamp
                    touch_file(&sidecar_path)?;
                    *skipped.lock().unwrap() += 1;
                    Ok(())
                }
            })?;
            return Ok(());
        }

        *skipped.lock().unwrap() += 1;
        pb.set_message(format!("Skipped: {}", file_name.bright_black()));
        return Ok(());
    }
    let relative_path = file_path.strip_prefix(".").unwrap_or(file_path);

    // Get file system metadata
    let (file_size, modified) = extract_file_metadata(file_path)?;

    let content = generate_sidecar_content(
        file_path,
        &file_name,
        &relative_path.to_string_lossy(),
        file_size,
        modified.as_deref(),
    );

    fs::write(&sidecar_path, content)?;
    pb.set_message(format!("Created: {}", file_name.green()));
    *created.lock().unwrap() += 1;

    Ok(())
}

fn get_sidecar_path(media_path: &Path) -> PathBuf {
    let mut sidecar_path = media_path.to_path_buf();
    let current_name = sidecar_path.file_name().unwrap().to_string_lossy();
    let new_name = format!("{current_name}.{SIDECAR_EXTENSION}");
    sidecar_path.set_file_name(new_name);
    sidecar_path
}

// Helper functions
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
            .template("{spinner:.cyan} {msg}")
            .unwrap()
            .tick_strings(SPINNER_CHARS),
    );
    spinner
}

fn create_progress_bar(total: u64) -> ProgressBar {
    let pb = ProgressBar::new(total);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.cyan} [{bar:40.cyan/blue}] {pos}/{len} {msg}")
            .unwrap()
            .progress_chars("█▓░"),
    );
    pb
}

fn print_update_summary(created: u32, updated: u32, skipped: u32) {
    println!("\n{} {}", "✓".green().bold(), "Update complete!".bold());
    println!(
        "  {} {} new sidecar files",
        "Created:".bright_black(),
        created.to_string().green().bold()
    );
    println!(
        "  {} {} existing files",
        "Updated:".bright_black(),
        updated.to_string().blue().bold()
    );
    println!(
        "  {} {} files {}",
        "Skipped:".bright_black(),
        skipped.to_string().yellow().bold(),
        "(already have sidecars)".bright_black()
    );
}

fn extract_file_metadata(path: &Path) -> Result<(u64, Option<String>), Box<dyn Error>> {
    let metadata = std::fs::metadata(path)?;
    let file_size = metadata.len();

    let modified = metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .and_then(|duration| {
            chrono::DateTime::<chrono::Utc>::from_timestamp(duration.as_secs() as i64, 0)
        })
        .map(|dt| dt.format("%Y-%m-%d %H:%M:%S UTC").to_string());

    Ok((file_size, modified))
}

fn generate_sidecar_content(
    file_path: &Path,
    file_name: &str,
    relative_path: &str,
    file_size: u64,
    modified: Option<&str>,
) -> String {
    let extension = file_path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase());

    match extension.as_deref() {
        Some("flac") | Some("wav") => {
            match read_audio_metadata(file_path) {
                Ok(metadata) => {
                    // Generate sidecar with metadata
                    templates::generate_audio_sidecar_with_metadata(&SidecarMetadata {
                        file_name,
                        file_path: relative_path,
                        sample_rate: metadata.sample_rate,
                        channels: metadata.channels,
                        bits_per_sample: metadata.bits_per_sample,
                        duration_seconds: metadata.duration_seconds,
                        file_size,
                        modified,
                    })
                }
                Err(e) => {
                    eprintln!(
                        "  {} Could not read metadata from {}: {}",
                        "Warning:".yellow(),
                        file_name.yellow(),
                        e.to_string().bright_black()
                    );
                    templates::generate_minimal_sidecar_with_fs_metadata(
                        file_name,
                        relative_path,
                        file_size,
                        modified,
                    )
                }
            }
        }
        _ => {
            // Unsupported audio format - create minimal sidecar
            templates::generate_minimal_sidecar_with_fs_metadata(
                file_name,
                relative_path,
                file_size,
                modified,
            )
        }
    }
}

fn touch_file(path: &Path) -> Result<(), Box<dyn Error>> {
    let now = SystemTime::now();
    fs::File::options()
        .create(true)
        .truncate(false)
        .write(true)
        .open(path)?
        .set_modified(now)?;
    Ok(())
}

fn offer_metadata_update(
    audio_path: &Path,
    sidecar_path: &Path,
    updated: &Arc<Mutex<u32>>,
) -> Result<bool, Box<dyn Error>> {
    let file_name = audio_path.file_name().unwrap().to_string_lossy();

    println!(
        "\n{} Audio file '{}' is newer than its sidecar",
        "⚠".yellow(),
        file_name.cyan()
    );

    // Read current sidecar content
    let sidecar_content = fs::read_to_string(sidecar_path)?;
    let (yaml_data, markdown_content) = if sidecar_content.starts_with("---\n") {
        let end_index = fs::read_to_string(sidecar_path)?[4..]
            .find("\n---\n")
            .ok_or("Invalid YAML frontmatter")?;
        let yaml_content = &sidecar_content[4..4 + end_index];
        let markdown = &sidecar_content[4 + end_index + 5..]; // Skip past "\n---\n"

        let yaml: HashMap<String, serde_yaml::Value> = serde_yaml::from_str(yaml_content)?;
        (yaml, markdown.to_string())
    } else {
        return Err("Sidecar file has no YAML frontmatter".into());
    };

    // Get new metadata from audio file
    let (new_file_size, new_modified) = extract_file_metadata(audio_path)?;
    let mut changes = Vec::new();

    // Check file size
    if let Some(old_size) = yaml_data.get("file_size").and_then(|v| v.as_u64())
        && old_size != new_file_size
    {
        changes.push(format!(
            "  file_size: {} → {}",
            old_size.to_string().red(),
            new_file_size.to_string().green()
        ));
    }

    // Check modified date
    if let Some(new_mod) = &new_modified
        && let Some(old_mod) = yaml_data.get("modified").and_then(|v| v.as_str())
        && old_mod != new_mod
    {
        changes.push(format!(
            "  modified: {} → {}",
            old_mod.red(),
            new_mod.green()
        ));
    }

    // Try to get audio metadata for duration/sample rate/etc
    let extension = audio_path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase());

    let audio_metadata = match extension.as_deref() {
        Some("flac") | Some("wav") => read_audio_metadata(audio_path).ok(),
        _ => None,
    };

    if let Some(ref metadata) = audio_metadata {
        // Check duration
        if let Some(new_duration) = metadata.duration_seconds
            && let Some(old_duration) = yaml_data.get("duration_seconds").and_then(|v| v.as_f64())
            && (old_duration - new_duration).abs() > 0.01
        {
            changes.push(format!(
                "  duration_seconds: {} → {}",
                format!("{old_duration:.2}").red(),
                format!("{new_duration:.2}").green()
            ));
        }

        // Check sample rate
        if let Some(old_rate) = yaml_data.get("sample_rate").and_then(|v| v.as_u64())
            && old_rate != metadata.sample_rate as u64
        {
            changes.push(format!(
                "  sample_rate: {} → {}",
                old_rate.to_string().red(),
                metadata.sample_rate.to_string().green()
            ));
        }

        // Check channels
        if let Some(old_channels) = yaml_data.get("channels").and_then(|v| v.as_u64())
            && old_channels != metadata.channels as u64
        {
            changes.push(format!(
                "  channels: {} → {}",
                old_channels.to_string().red(),
                metadata.channels.to_string().green()
            ));
        }

        // Check bits per sample
        if let Some(old_bits) = yaml_data.get("bits_per_sample").and_then(|v| v.as_u64())
            && old_bits != metadata.bits_per_sample as u64
        {
            changes.push(format!(
                "  bits_per_sample: {} → {}",
                old_bits.to_string().red(),
                metadata.bits_per_sample.to_string().green()
            ));
        }
    }

    if changes.is_empty() {
        println!("  No metadata changes detected (timestamps differ but content is the same)");
        print!("  Touch the sidecar file to update its timestamp? (y/n): ");
        io::stdout().flush()?;

        let mut response = String::new();
        io::stdin().read_line(&mut response)?;

        if response.trim().to_lowercase() == "y" {
            touch_file(sidecar_path)?;
            println!("  {} Touched sidecar file", "✓".green());
        }
        return Ok(false);
    }

    println!("\n  Changes detected:");
    for change in &changes {
        println!("{change}");
    }

    print!("\n  Update these fields in the sidecar? (y/n): ");
    io::stdout().flush()?;

    let mut response = String::new();
    io::stdin().read_line(&mut response)?;

    if response.trim().to_lowercase() == "y" {
        // Update the YAML data
        let mut updated_yaml = yaml_data.clone();

        updated_yaml.insert(
            "file_size".to_string(),
            serde_yaml::Value::Number(new_file_size.into()),
        );
        if let Some(new_mod) = new_modified {
            updated_yaml.insert("modified".to_string(), serde_yaml::Value::String(new_mod));
        }

        if let Some(metadata) = audio_metadata {
            if let Some(duration) = metadata.duration_seconds {
                updated_yaml.insert(
                    "duration_seconds".to_string(),
                    serde_yaml::Value::Number(serde_yaml::Number::from(duration)),
                );
            }
            updated_yaml.insert(
                "sample_rate".to_string(),
                serde_yaml::Value::Number(metadata.sample_rate.into()),
            );
            updated_yaml.insert(
                "channels".to_string(),
                serde_yaml::Value::Number(metadata.channels.into()),
            );
            updated_yaml.insert(
                "bits_per_sample".to_string(),
                serde_yaml::Value::Number(metadata.bits_per_sample.into()),
            );
        }

        // Reconstruct the file
        let yaml_string = serde_yaml::to_string(&updated_yaml)?;
        let new_content = format!("---\n{yaml_string}---\n{markdown_content}");

        fs::write(sidecar_path, new_content)?;
        println!("  {} Updated metadata", "✓".green());
        *updated.lock().unwrap() += 1;

        Ok(true)
    } else {
        Ok(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_get_sidecar_path() {
        let path = Path::new("/test/audio.wav");
        let sidecar = get_sidecar_path(path);
        assert_eq!(sidecar, Path::new("/test/audio.wav.md"));
    }

    #[test]
    fn test_get_sidecar_path_with_multiple_dots() {
        let path = Path::new("/test/my.audio.file.wav");
        let sidecar = get_sidecar_path(path);
        assert_eq!(sidecar, Path::new("/test/my.audio.file.wav.md"));
    }

    #[test]
    fn test_get_sidecar_path_no_extension() {
        let path = Path::new("/test/audiofile");
        let sidecar = get_sidecar_path(path);
        assert_eq!(sidecar, Path::new("/test/audiofile.md"));
    }

    #[test]
    fn test_should_skip_directory() {
        assert!(should_skip_directory("node_modules"));
        assert!(should_skip_directory(".git"));
        assert!(should_skip_directory("temp"));
        assert!(!should_skip_directory("src"));
        assert!(!should_skip_directory("audio"));
    }

    #[test]
    fn test_is_hidden_file() {
        assert!(is_hidden_file(Path::new(".hidden")));
        assert!(is_hidden_file(Path::new("/path/.hidden")));
        assert!(is_hidden_file(Path::new(".DS_Store")));
        assert!(!is_hidden_file(Path::new("visible")));
        assert!(!is_hidden_file(Path::new("/path/visible")));
    }

    #[test]
    fn test_create_progress_spinner() {
        let spinner = create_progress_spinner();
        // Just verify it creates without panicking
        spinner.set_message("Test");
        spinner.finish();
    }

    #[test]
    fn test_create_progress_bar() {
        let pb = create_progress_bar(100);
        // Just verify it creates without panicking
        pb.set_position(50);
        pb.finish();
    }

    #[test]
    fn test_audio_extensions() {
        let extensions: HashSet<&str> = AUDIO_EXTENSIONS.iter().cloned().collect();
        assert!(extensions.contains("wav"));
        assert!(extensions.contains("flac"));
        assert!(extensions.contains("mp3"));
        assert!(extensions.contains("aiff"));
        assert!(extensions.contains("m4a"));
        assert_eq!(extensions.len(), 5);
    }

    #[test]
    fn test_count_audio_files_empty_dir() {
        let temp_dir = TempDir::new().unwrap();
        let extensions: HashSet<&str> = AUDIO_EXTENSIONS.iter().cloned().collect();

        let zimignore = ZimIgnore::new();
        let count = count_audio_files(temp_dir.path(), &extensions, &zimignore).unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn test_count_audio_files_with_audio() {
        let temp_dir = TempDir::new().unwrap();
        let extensions: HashSet<&str> = AUDIO_EXTENSIONS.iter().cloned().collect();

        // Create some audio files
        fs::write(temp_dir.path().join("test.wav"), b"fake").unwrap();
        fs::write(temp_dir.path().join("test.flac"), b"fake").unwrap();
        fs::write(temp_dir.path().join("test.mp3"), b"fake").unwrap();

        // Create non-audio files
        fs::write(temp_dir.path().join("test.txt"), b"fake").unwrap();
        fs::write(temp_dir.path().join("README.md"), b"fake").unwrap();

        let zimignore = ZimIgnore::new();
        let count = count_audio_files(temp_dir.path(), &extensions, &zimignore).unwrap();
        assert_eq!(count, 3);
    }

    #[test]
    fn test_count_audio_files_skip_hidden() {
        let temp_dir = TempDir::new().unwrap();
        let extensions: HashSet<&str> = AUDIO_EXTENSIONS.iter().cloned().collect();

        // Create visible and hidden audio files
        fs::write(temp_dir.path().join("visible.wav"), b"fake").unwrap();
        fs::write(temp_dir.path().join(".hidden.wav"), b"fake").unwrap();

        let zimignore = ZimIgnore::new();
        let count = count_audio_files(temp_dir.path(), &extensions, &zimignore).unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_count_audio_files_skip_directories() {
        let temp_dir = TempDir::new().unwrap();
        let extensions: HashSet<&str> = AUDIO_EXTENSIONS.iter().cloned().collect();

        // Create a normal directory with audio
        let normal_dir = temp_dir.path().join("music");
        fs::create_dir(&normal_dir).unwrap();
        fs::write(normal_dir.join("test.wav"), b"fake").unwrap();

        // Create a skip directory with audio
        let skip_dir = temp_dir.path().join("node_modules");
        fs::create_dir(&skip_dir).unwrap();
        fs::write(skip_dir.join("test.wav"), b"fake").unwrap();

        let zimignore = ZimIgnore::new();
        let count = count_audio_files(temp_dir.path(), &extensions, &zimignore).unwrap();
        assert_eq!(count, 1); // Only from normal_dir
    }

    #[test]
    fn test_extract_file_metadata() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        fs::write(&file_path, b"test content").unwrap();

        let result = extract_file_metadata(&file_path);
        assert!(result.is_ok());

        let (size, modified) = result.unwrap();
        assert_eq!(size, 12); // "test content" is 12 bytes
        assert!(modified.is_some());
        assert!(modified.unwrap().contains("UTC"));
    }

    #[test]
    fn test_touch_file() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.md");

        // Create a file
        fs::write(&file_path, b"test content").unwrap();

        // Get original modification time
        let original_metadata = fs::metadata(&file_path).unwrap();
        let original_modified = original_metadata.modified().unwrap();

        // Sleep briefly to ensure time difference
        std::thread::sleep(std::time::Duration::from_millis(10));

        // Touch the file
        let result = touch_file(&file_path);
        assert!(result.is_ok());

        // Check new modification time
        let new_metadata = fs::metadata(&file_path).unwrap();
        let new_modified = new_metadata.modified().unwrap();

        // Verify the file was touched (new time > old time)
        assert!(new_modified > original_modified);

        // Verify content unchanged
        let content = fs::read_to_string(&file_path).unwrap();
        assert_eq!(content, "test content");
    }
}
