use crate::config::Config;
use crate::media::metadata::read_audio_metadata;
use crate::templates::{self, SidecarMetadata};
use crate::wav_metadata;
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
use zim_studio::utils::parallel_scan;
use zim_studio::utils::sidecar::get_sidecar_path;
use zim_studio::zimignore::ZimIgnore;

// Constants
const AUDIO_EXTENSIONS: &[&str] = &["wav", "flac", "aiff", "mp3", "m4a"];
const SPINNER_CHARS: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

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

    // Load configuration with tag mappings
    let config = Arc::new(Config::load()?);

    println!(
        "{} {}",
        "Scanning project:".bright_black(),
        project_path.display().to_string().cyan()
    );

    // Get audio file extensions we want sidecars for
    let audio_extensions: HashSet<&str> = AUDIO_EXTENSIONS.iter().cloned().collect();

    // Load .zimignore files for this directory hierarchy
    let zimignore = ZimIgnore::load_for_directory(project_path);

    // Collect all audio files using parallel scanning
    let spinner = create_progress_spinner();
    spinner.set_message("Scanning for audio files...");

    let audio_files =
        parallel_scan::collect_audio_files(project_path, &audio_extensions, &zimignore)?;
    spinner.finish_and_clear();

    let total_files = audio_files.len();

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

    // Create project root cache for performance
    let project_cache = Arc::new(Mutex::new(HashMap::<PathBuf, Option<String>>::new()));

    let multi = MultiProgress::new();
    let pb = multi.add(create_progress_bar(total_files as u64));
    pb.set_message("Processing audio files...");

    // Process files sequentially but with parallel scanning already done
    // (Processing itself involves I/O and user interaction which can't be parallelized)
    for file_path in &audio_files {
        let result = process_media_file(
            file_path,
            &created_count,
            &skipped_count,
            &updated_count,
            &pb,
            &project_cache,
            &config,
        );

        if let Err(e) = result {
            eprintln!("{} {}", "Error:".red(), e);
        }

        pb.inc(1);
    }

    pb.finish_with_message("Done");

    let created = *created_count.lock().unwrap();
    let updated = *updated_count.lock().unwrap();
    let skipped = *skipped_count.lock().unwrap();

    print_update_summary(created, updated, skipped);

    Ok(())
}

/// Maximum depth to traverse when looking for project root
const MAX_PROJECT_TRAVERSAL_DEPTH: usize = 10;

/// Find the project root by looking for the nearest .zimignore file
///
/// # Example
/// ```
/// // Given a file at: /home/user/projects/my-song/mixes/final.wav
/// // With .zimignore at: /home/user/projects/my-song/.zimignore
/// // Returns: Some("my-song")
/// ```
fn find_project_root(file_path: &Path) -> Option<String> {
    // Start from the file's parent directory
    let mut current = file_path.parent();
    let mut depth = 0;

    while let Some(dir) = current {
        // Prevent excessive traversal
        if depth >= MAX_PROJECT_TRAVERSAL_DEPTH {
            // Reached maximum depth, stop searching to prevent infinite loops
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

/// Extract a clean title from a filename by removing the extension
fn extract_title_from_filename(filename: &str) -> String {
    // Remove extension(s) - handles cases like "my.song.wav"
    if let Some(dot_pos) = filename.rfind('.') {
        filename[..dot_pos].to_string()
    } else {
        filename.to_string()
    }
}

/// Determine the file type based on its directory within the project
/// Returns (singular_type, tag) e.g., ("edit", "edit") or ("source", "source")
fn determine_file_type(file_path: &Path) -> Option<(String, String)> {
    // Get the path components
    let components: Vec<&str> = file_path
        .components()
        .filter_map(|c| {
            if let std::path::Component::Normal(s) = c {
                s.to_str()
            } else {
                None
            }
        })
        .collect();

    // Look for known audio directories in the path
    // Start from the first directory after the project root and look for known patterns
    for component in components.iter().rev().skip(1) {
        // Skip the filename itself
        let dir = component.to_lowercase();

        // Map directory names to their singular forms and tags
        let (singular, tag) = match dir.as_str() {
            "mixes" => ("mix", "mix"),
            "mix" => ("mix", "mix"),
            "edits" => ("edit", "edit"),
            "edit" => ("edit", "edit"),
            "sources" => ("source", "source"),
            "source" => ("source", "source"),
            "recordings" => ("recording", "recording"),
            "recording" => ("recording", "recording"),
            "samples" => ("sample", "sample"),
            "sample" => ("sample", "sample"),
            "stems" => ("stem", "stem"),
            "stem" => ("stem", "stem"),
            "bounced" => ("bounce", "bounce"),
            "bounce" => ("bounce", "bounce"),
            "renders" => ("render", "render"),
            "render" => ("render", "render"),
            "masters" => ("master", "master"),
            "master" => ("master", "master"),
            "demos" => ("demo", "demo"),
            "demo" => ("demo", "demo"),
            "drafts" => ("draft", "draft"),
            "draft" => ("draft", "draft"),
            "ideas" => ("idea", "idea"),
            "idea" => ("idea", "idea"),
            "loops" => ("loop", "loop"),
            "loop" => ("loop", "loop"),
            "takes" => ("take", "take"),
            "take" => ("take", "take"),
            _ => {
                // Not a known audio directory, continue searching
                continue;
            }
        };

        return Some((singular.to_string(), tag.to_string()));
    }

    None
}

/// Determine whether to use "a" or "an" based on the word
///
/// # Example
/// ```
/// assert_eq!(get_article("edit"), "an");
/// assert_eq!(get_article("mix"), "a");
/// assert_eq!(get_article("source"), "a");
/// ```
fn get_article(word: &str) -> &'static str {
    if word.is_empty() {
        return "a";
    }

    // Safe handling of first character
    let first_char = match word.chars().next() {
        Some(c) => c.to_ascii_lowercase(),
        None => return "a",
    };

    // Check for vowel sounds (simplified - doesn't handle all edge cases)
    match first_char {
        'a' | 'e' | 'i' | 'o' | 'u' => "an",
        // Special cases for words that start with silent 'h'
        'h' if word.to_lowercase().starts_with("hour") => "an",
        _ => "a",
    }
}

/// Generate a smart description based on file type and project
fn generate_description(file_type: Option<&str>, project: Option<&str>) -> String {
    match (file_type, project) {
        (Some(ft), Some(proj)) => {
            let article = get_article(ft);
            format!("{article} {ft} for {proj}")
        }
        (Some(ft), None) => {
            let article = get_article(ft);
            format!("{article} {ft}")
        }
        _ => String::new(),
    }
}

#[allow(clippy::too_many_arguments)]
fn process_media_file(
    file_path: &Path,
    created: &Arc<Mutex<u32>>,
    skipped: &Arc<Mutex<u32>>,
    updated: &Arc<Mutex<u32>>,
    pb: &ProgressBar,
    project_cache: &Arc<Mutex<HashMap<PathBuf, Option<String>>>>,
    config: &Arc<Config>,
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

    // Find the project name (with caching)
    let project_name = {
        let mut cache = project_cache.lock().unwrap();
        let parent = file_path.parent().unwrap_or(Path::new("."));
        cache
            .entry(parent.to_path_buf())
            .or_insert_with(|| find_project_root(file_path))
            .clone()
    };

    let content = generate_sidecar_content(
        file_path,
        &file_name,
        &relative_path.to_string_lossy(),
        file_size,
        modified.as_deref(),
        project_name.as_deref(),
        config,
    );

    fs::write(&sidecar_path, content)?;
    pb.set_message(format!("Created: {}", file_name.green()));
    *created.lock().unwrap() += 1;

    Ok(())
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

#[allow(clippy::too_many_arguments)]
fn generate_sidecar_content(
    file_path: &Path,
    file_name: &str,
    relative_path: &str,
    file_size: u64,
    modified: Option<&str>,
    project: Option<&str>,
    config: &Config,
) -> String {
    // Calculate smart defaults
    let title = extract_title_from_filename(file_name);
    let file_type_info = determine_file_type(Path::new(relative_path));
    let (file_type, tag) = file_type_info
        .as_ref()
        .map(|(t, tag)| (t.as_str(), tag.as_str()))
        .unwrap_or(("", ""));
    let description = generate_description(Some(file_type).filter(|s| !s.is_empty()), project);

    // Create tags list with the file type tag if available
    let mut tags = if !tag.is_empty() {
        vec![tag.to_string()]
    } else {
        vec![]
    };

    // Add tags based on filename mappings from config
    let filename_lower = file_name.to_lowercase();
    for (pattern, tag_value) in &config.tag_mappings {
        if filename_lower.contains(&pattern.to_lowercase()) && !tags.contains(tag_value) {
            tags.push(tag_value.clone());
        }
    }

    let extension = file_path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase());

    // For WAV files, check/add ZIM metadata first
    let uuid = if extension.as_deref() == Some("wav") {
        // Check if file already has ZIM metadata
        let existing_metadata = wav_metadata::read_metadata(file_path).ok().flatten();

        if let Some(metadata) = existing_metadata {
            // File already has metadata, just get the UUID
            Some(metadata.uuid)
        } else {
            // No metadata yet - tag the file in-place
            if let Ok(abs_path) = std::fs::canonicalize(file_path) {
                let project_name = project.unwrap_or("unknown");
                let mut new_metadata =
                    wav_metadata::ZimMetadata::new_original(project_name, &abs_path);

                // Calculate MD5 and set it
                if let Ok(md5) = wav_metadata::calculate_audio_md5(file_path) {
                    new_metadata.audio_md5 = md5;
                }

                // Create temp file for safety
                let temp_path = file_path.with_extension("wav.tmp");

                // Write metadata to temp file
                if wav_metadata::write_metadata(file_path, &temp_path, &new_metadata).is_ok() {
                    // Replace original with temp file
                    if std::fs::rename(&temp_path, file_path).is_ok() {
                        Some(new_metadata.uuid)
                    } else {
                        // Clean up temp file if rename failed
                        let _ = std::fs::remove_file(&temp_path);
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            }
        }
    } else {
        None
    };

    match extension.as_deref() {
        Some("flac") | Some("wav") => {
            match read_audio_metadata(file_path) {
                Ok(metadata) => {
                    // Generate sidecar with metadata
                    templates::generate_audio_sidecar_with_metadata(&SidecarMetadata {
                        file_name,
                        file_path: relative_path,
                        title: &title,
                        description: &description,
                        tags: &tags,
                        sample_rate: metadata.sample_rate,
                        channels: metadata.channels,
                        bits_per_sample: metadata.bits_per_sample,
                        duration_seconds: metadata.duration_seconds,
                        file_size,
                        modified,
                        project,
                        uuid: uuid.as_deref(),
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
                        &title,
                        &description,
                        &tags,
                        file_size,
                        modified,
                        project,
                        uuid.as_deref(),
                    )
                }
            }
        }
        _ => {
            // Unsupported audio format - create minimal sidecar
            templates::generate_minimal_sidecar_with_fs_metadata(
                file_name,
                relative_path,
                &title,
                &description,
                &tags,
                file_size,
                modified,
                project,
                None,
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
        assert!(parallel_scan::should_skip_directory("node_modules"));
        assert!(parallel_scan::should_skip_directory(".git"));
        assert!(parallel_scan::should_skip_directory("temp"));
        assert!(!parallel_scan::should_skip_directory("src"));
        assert!(!parallel_scan::should_skip_directory("audio"));
    }

    #[test]
    fn test_is_hidden_file() {
        assert!(parallel_scan::is_hidden_file(Path::new(".hidden")));
        assert!(parallel_scan::is_hidden_file(Path::new("/path/.hidden")));
        assert!(parallel_scan::is_hidden_file(Path::new(".DS_Store")));
        assert!(!parallel_scan::is_hidden_file(Path::new("visible")));
        assert!(!parallel_scan::is_hidden_file(Path::new("/path/visible")));
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
        let files =
            parallel_scan::collect_audio_files(temp_dir.path(), &extensions, &zimignore).unwrap();
        assert_eq!(files.len(), 0);
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
        let files =
            parallel_scan::collect_audio_files(temp_dir.path(), &extensions, &zimignore).unwrap();
        assert_eq!(files.len(), 3);
    }

    #[test]
    fn test_count_audio_files_skip_hidden() {
        let temp_dir = TempDir::new().unwrap();
        let extensions: HashSet<&str> = AUDIO_EXTENSIONS.iter().cloned().collect();

        // Create visible and hidden audio files
        fs::write(temp_dir.path().join("visible.wav"), b"fake").unwrap();
        fs::write(temp_dir.path().join(".hidden.wav"), b"fake").unwrap();

        let zimignore = ZimIgnore::new();
        let files =
            parallel_scan::collect_audio_files(temp_dir.path(), &extensions, &zimignore).unwrap();
        assert_eq!(files.len(), 1);
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
        let files =
            parallel_scan::collect_audio_files(temp_dir.path(), &extensions, &zimignore).unwrap();
        assert_eq!(files.len(), 1); // Only from normal_dir
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

    #[test]
    fn test_extract_title_from_filename() {
        assert_eq!(extract_title_from_filename("song.wav"), "song");
        assert_eq!(
            extract_title_from_filename("my.great.song.flac"),
            "my.great.song"
        );
        assert_eq!(
            extract_title_from_filename("Final_Mix_v2.wav"),
            "Final_Mix_v2"
        );
        assert_eq!(extract_title_from_filename("no_extension"), "no_extension");
        assert_eq!(extract_title_from_filename(""), "");
    }

    #[test]
    fn test_get_article() {
        // Vowel starts
        assert_eq!(get_article("edit"), "an");
        assert_eq!(get_article("idea"), "an");
        assert_eq!(get_article("audio"), "an");
        assert_eq!(get_article("outro"), "an");
        assert_eq!(get_article("underwater"), "an");

        // Consonant starts
        assert_eq!(get_article("mix"), "a");
        assert_eq!(get_article("source"), "a");
        assert_eq!(get_article("demo"), "a");
        assert_eq!(get_article("sample"), "a");

        // Edge cases
        assert_eq!(get_article(""), "a");
        assert_eq!(get_article("hour"), "an"); // Silent h
        assert_eq!(get_article("house"), "a"); // Pronounced h

        // Unicode handling
        assert_eq!(get_article("émigré"), "a"); // Non-ASCII
    }

    #[test]
    fn test_determine_file_type() {
        use std::path::PathBuf;

        // Standard audio directories
        assert_eq!(
            determine_file_type(&PathBuf::from("mixes/final.wav")),
            Some(("mix".to_string(), "mix".to_string()))
        );
        assert_eq!(
            determine_file_type(&PathBuf::from("edits/intro.wav")),
            Some(("edit".to_string(), "edit".to_string()))
        );
        assert_eq!(
            determine_file_type(&PathBuf::from("sources/guitar.wav")),
            Some(("source".to_string(), "source".to_string()))
        );
        assert_eq!(
            determine_file_type(&PathBuf::from("samples/kick.wav")),
            Some(("sample".to_string(), "sample".to_string()))
        );

        // Nested directories should find the audio directory
        assert_eq!(
            determine_file_type(&PathBuf::from("sources/day1/guitar.wav")),
            Some(("source".to_string(), "source".to_string()))
        );
        assert_eq!(
            determine_file_type(&PathBuf::from("mixes/old/2024/final.wav")),
            Some(("mix".to_string(), "mix".to_string()))
        );

        // Non-audio directories
        assert_eq!(determine_file_type(&PathBuf::from("random/file.wav")), None);
        assert_eq!(determine_file_type(&PathBuf::from("file.wav")), None);

        // Project with multiple levels
        assert_eq!(
            determine_file_type(&PathBuf::from("project/stems/drums.wav")),
            Some(("stem".to_string(), "stem".to_string()))
        );
    }

    #[test]
    fn test_generate_description() {
        assert_eq!(
            generate_description(Some("mix"), Some("my-project")),
            "a mix for my-project"
        );
        assert_eq!(
            generate_description(Some("edit"), Some("cool-song")),
            "an edit for cool-song"
        );
        assert_eq!(generate_description(Some("source"), None), "a source");
        assert_eq!(generate_description(None, Some("project")), "");
        assert_eq!(generate_description(None, None), "");
    }

    #[test]
    fn test_find_project_root() {
        use std::fs;
        use tempfile::TempDir;

        // Create a temporary directory structure
        let temp_dir = TempDir::new().unwrap();
        let project_dir = temp_dir.path().join("my-project");
        let mixes_dir = project_dir.join("mixes");
        let nested_dir = mixes_dir.join("old");

        fs::create_dir_all(&nested_dir).unwrap();
        fs::write(project_dir.join(".zimignore"), "# test").unwrap();

        let file_path = nested_dir.join("test.wav");
        fs::write(&file_path, "").unwrap();

        // Test finding project root
        let result = find_project_root(&file_path);
        assert_eq!(result, Some("my-project".to_string()));

        // Test with no .zimignore
        let orphan_dir = temp_dir.path().join("orphan");
        fs::create_dir_all(&orphan_dir).unwrap();
        let orphan_file = orphan_dir.join("test.wav");
        fs::write(&orphan_file, "").unwrap();

        let result = find_project_root(&orphan_file);
        assert_eq!(result, None);
    }
}
