use owo_colors::OwoColorize;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::error::Error;
use std::fs;
use std::path::Path;
use zim_studio::constants::{AUDIO_EXTENSIONS, SIDECAR_EXTENSION};
use zim_studio::utils::{progress::create_progress_spinner, validation::validate_path_exists};

/// Track metadata extracted from sidecar file
#[derive(Debug, Clone, Serialize, Deserialize)]
struct TrackInfo {
    file: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    duration: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    sample_rate: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    channels: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    bit_depth: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tags: Option<Vec<String>>,
}

/// Album summary metadata
#[derive(Debug, Serialize, Deserialize)]
struct AlbumInfo {
    total_tracks: usize,
    total_duration: Option<f64>,
    generated: String,
}

/// Root structure for index YAML
#[derive(Debug, Serialize, Deserialize)]
struct IndexData {
    album: AlbumInfo,
    tracks: Vec<TrackInfo>,
}

pub fn handle_index(project_path: &str) -> Result<(), Box<dyn Error>> {
    let project_path = Path::new(project_path);

    validate_path_exists(project_path)?;

    println!(
        "{} {}",
        "Generating index for:".bright_black(),
        project_path.display().to_string().cyan()
    );
    println!();

    let spinner = create_progress_spinner();
    spinner.set_message("Scanning for audio files with sidecars...");

    // Collect audio files with sidecars
    let mut tracks = Vec::new();
    collect_tracks(project_path, &mut tracks)?;

    spinner.finish_and_clear();

    if tracks.is_empty() {
        println!(
            "{} No audio files with sidecars found in {}",
            "⚠".yellow(),
            project_path.display().to_string().cyan()
        );
        return Ok(());
    }

    // Sort tracks by filename
    tracks.sort_by(|a, b| a.file.cmp(&b.file));

    // Calculate total duration
    let total_duration: Option<f64> = tracks
        .iter()
        .try_fold(0.0, |acc, track| track.duration.map(|d| acc + d));

    // Generate timestamp
    let now = chrono::Utc::now();
    let generated = now.to_rfc3339();

    // Create index data
    let index_data = IndexData {
        album: AlbumInfo {
            total_tracks: tracks.len(),
            total_duration,
            generated: generated.clone(),
        },
        tracks: tracks.clone(),
    };

    // Generate index.yml content
    let content = serde_yaml::to_string(&index_data)?;

    // Write index.yml
    let index_path = project_path.join("index.yml");
    fs::write(&index_path, content)?;

    println!(
        "{} {} Created {}",
        "✓".green().bold(),
        "Index generated:".green(),
        index_path.display().to_string().cyan()
    );
    println!(
        "  {} {} tracks",
        "Tracks:".bright_black(),
        tracks.len().to_string().cyan()
    );
    if let Some(duration) = total_duration {
        let total_mins = (duration / 60.0) as u32;
        let total_secs = (duration % 60.0) as u32;
        println!(
            "  {} {}:{:02}",
            "Total Duration:".bright_black(),
            total_mins.to_string().cyan(),
            total_secs.to_string().cyan()
        );
    }

    Ok(())
}

fn collect_tracks(dir: &Path, tracks: &mut Vec<TrackInfo>) -> Result<(), Box<dyn Error>> {
    let entries = fs::read_dir(dir)?;

    for entry in entries {
        let entry = entry?;
        let path = entry.path();

        // Skip hidden files
        if let Some(name) = path.file_name()
            && name.to_string_lossy().starts_with('.')
        {
            continue;
        }

        if path.is_file() {
            // Check if it's an audio file
            if let Some(ext) = path.extension()
                && let Some(ext_str) = ext.to_str()
            {
                let ext_lower = ext_str.to_lowercase();
                if AUDIO_EXTENSIONS.contains(&ext_lower.as_str()) {
                    // Check for sidecar
                    let sidecar_path =
                        path.with_extension(format!("{ext_str}.{SIDECAR_EXTENSION}"));
                    if sidecar_path.exists() {
                        // Read track info from sidecar
                        if let Ok(track_info) = read_track_info(&path, &sidecar_path) {
                            tracks.push(track_info);
                        }
                    }
                }
            }
        }
    }

    Ok(())
}

fn read_track_info(audio_path: &Path, sidecar_path: &Path) -> Result<TrackInfo, Box<dyn Error>> {
    let content = fs::read_to_string(sidecar_path)?;

    // Parse YAML frontmatter
    if !content.starts_with("---\n") {
        return Err("No YAML frontmatter found".into());
    }

    let end_index = content[4..]
        .find("\n---\n")
        .ok_or("Invalid YAML frontmatter")?;
    let yaml_content = &content[4..4 + end_index];

    let yaml: HashMap<String, serde_yaml::Value> = serde_yaml::from_str(yaml_content)?;

    // Extract relevant fields
    let file_name = audio_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_string();

    let title = yaml
        .get("title")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let description = yaml
        .get("description")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let duration = yaml.get("duration").and_then(|v| v.as_f64());

    let sample_rate = yaml
        .get("sample_rate")
        .and_then(|v| v.as_u64())
        .map(|v| v as u32);

    let channels = yaml
        .get("channels")
        .and_then(|v| v.as_u64())
        .map(|v| v as u16);

    let bit_depth = yaml
        .get("bit_depth")
        .and_then(|v| v.as_u64())
        .map(|v| v as u16);

    let tags = yaml.get("tags").and_then(|v| v.as_sequence()).map(|seq| {
        seq.iter()
            .filter_map(|v| v.as_str())
            .map(|s| s.to_string())
            .collect()
    });

    Ok(TrackInfo {
        file: file_name,
        title,
        description,
        duration,
        sample_rate,
        channels,
        bit_depth,
        tags,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_index_data_serialization() {
        let data = IndexData {
            album: AlbumInfo {
                total_tracks: 2,
                total_duration: Some(300.0),
                generated: "2024-01-15T10:30:00Z".to_string(),
            },
            tracks: vec![
                TrackInfo {
                    file: "01-intro.flac".to_string(),
                    title: Some("Introduction".to_string()),
                    description: Some("Opening track".to_string()),
                    duration: Some(120.5),
                    sample_rate: Some(44100),
                    channels: Some(2),
                    bit_depth: Some(16),
                    tags: Some(vec!["intro".to_string()]),
                },
                TrackInfo {
                    file: "02-main.flac".to_string(),
                    title: Some("Main Theme".to_string()),
                    description: None,
                    duration: Some(179.5),
                    sample_rate: Some(44100),
                    channels: Some(2),
                    bit_depth: Some(16),
                    tags: None,
                },
            ],
        };

        // Serialize to YAML
        let yaml = serde_yaml::to_string(&data).unwrap();

        // Verify YAML contains expected data
        assert!(yaml.contains("total_tracks: 2"));
        assert!(yaml.contains("total_duration: 300"));
        assert!(yaml.contains("01-intro.flac"));
        assert!(yaml.contains("02-main.flac"));
        assert!(yaml.contains("Introduction"));
        assert!(yaml.contains("Main Theme"));
        assert!(yaml.contains("Opening track"));
        assert!(yaml.contains("sample_rate: 44100"));

        // Verify it can be deserialized back
        let deserialized: IndexData = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(deserialized.album.total_tracks, 2);
        assert_eq!(deserialized.tracks.len(), 2);
    }
}
