//! Utilities for working with sidecar metadata files.
//!
//! This module provides functions for constructing sidecar paths and cloning/updating
//! sidecar files when audio files are copied or excerpted.

use crate::constants::SIDECAR_EXTENSION;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

/// Constructs the sidecar file path for a given audio file.
///
/// # Example
/// ```ignore
/// let audio = Path::new("/music/track.wav");
/// let sidecar = get_sidecar_path(audio);
/// assert_eq!(sidecar, Path::new("/music/track.wav.md"));
/// ```
pub fn get_sidecar_path(audio_path: &Path) -> PathBuf {
    let mut sidecar_path = audio_path.to_path_buf();
    let current_name = sidecar_path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy();
    let new_name = format!("{current_name}.{SIDECAR_EXTENSION}");
    sidecar_path.set_file_name(new_name);
    sidecar_path
}

/// Mode for cloning sidecar files
#[derive(Debug, Clone)]
pub enum SidecarCloneMode {
    /// Copy sidecar and update file/path fields only
    FullCopy,
    /// Copy sidecar and add selection/excerpt metadata
    Selection {
        start_time: f32, // seconds
        end_time: f32,   // seconds
        duration: f32,   // total file duration in seconds
    },
}

/// Clone a sidecar file from source audio to destination audio.
///
/// This function handles:
/// - Constructing sidecar paths automatically
/// - Updating file/path fields to match destination
/// - Adding excerpt metadata for selections
/// - Preserving existing tags and metadata
/// - Gracefully handling missing source sidecars
///
/// # Arguments
/// * `source_audio` - Path to the source audio file
/// * `dest_audio` - Path to the destination audio file
/// * `mode` - Clone mode (full copy or selection)
/// * `tags_fallback` - Tags to use if source sidecar doesn't exist
///
/// # Returns
/// Returns `Ok(())` if successful, or an error if sidecar creation fails.
/// If the source sidecar doesn't exist, this returns `Ok(())` without creating a destination sidecar.
pub fn clone_sidecar(
    source_audio: &Path,
    dest_audio: &Path,
    mode: SidecarCloneMode,
    tags_fallback: Option<&[String]>,
) -> Result<(), Box<dyn Error>> {
    let source_sidecar = get_sidecar_path(source_audio);
    let dest_sidecar = get_sidecar_path(dest_audio);

    // If source sidecar doesn't exist, nothing to clone
    if !source_sidecar.exists() {
        return Ok(());
    }

    // Get destination metadata
    let dest_filename = dest_audio
        .file_name()
        .and_then(|f| f.to_str())
        .unwrap_or("Unknown");
    let dest_dir = dest_audio.parent().and_then(|p| p.to_str()).unwrap_or(".");

    // Read source sidecar
    let original_content = fs::read_to_string(&source_sidecar)?;

    let content = match mode {
        SidecarCloneMode::FullCopy => clone_full_copy(&original_content, dest_filename, dest_dir)?,
        SidecarCloneMode::Selection {
            start_time,
            end_time,
            duration,
        } => clone_selection(
            &original_content,
            source_audio,
            dest_filename,
            dest_dir,
            start_time,
            end_time,
            duration,
            tags_fallback,
        )?,
    };

    fs::write(&dest_sidecar, content)?;
    log::info!("Created sidecar file: {}", dest_sidecar.display());

    Ok(())
}

/// Clone sidecar for a full file copy (update file/path fields only)
fn clone_full_copy(
    original_content: &str,
    dest_filename: &str,
    dest_dir: &str,
) -> Result<String, Box<dyn Error>> {
    if let Some(frontmatter_end) = original_content.find("\n---\n") {
        // Has YAML frontmatter - parse and update file/path fields
        let yaml_section = &original_content[..frontmatter_end];
        let markdown_section = &original_content[frontmatter_end + 5..]; // Skip "\n---\n"

        // Parse original YAML to preserve all fields
        if let Ok(mut yaml_value) = serde_yaml::from_str::<serde_yaml::Value>(yaml_section)
            && let Some(yaml_map) = yaml_value.as_mapping_mut()
        {
            // Update file and path fields
            yaml_map.insert(
                serde_yaml::Value::String("file".to_string()),
                serde_yaml::Value::String(dest_filename.to_string()),
            );
            yaml_map.insert(
                serde_yaml::Value::String("path".to_string()),
                serde_yaml::Value::String(dest_dir.to_string()),
            );

            // Serialize back to YAML
            let updated_yaml = serde_yaml::to_string(&yaml_value)?;
            return Ok(format!("---\n{updated_yaml}---\n\n{markdown_section}"));
        }
    }

    // No frontmatter or failed to parse - copy as-is
    Ok(original_content.to_string())
}

/// Clone sidecar for a selection/excerpt (add provenance metadata)
#[allow(clippy::too_many_arguments)]
fn clone_selection(
    original_content: &str,
    source_audio: &Path,
    dest_filename: &str,
    dest_dir: &str,
    start_time: f32,
    end_time: f32,
    _duration: f32,
    tags_fallback: Option<&[String]>,
) -> Result<String, Box<dyn Error>> {
    let source_filename = source_audio
        .file_name()
        .and_then(|f| f.to_str())
        .unwrap_or("unknown source");
    let source_path = source_audio.to_str().unwrap_or("unknown");

    // Calculate time ranges
    let start_secs = start_time as u32;
    let end_secs = end_time as u32;
    let selection_duration = end_secs - start_secs;

    let start_mins = start_secs / 60;
    let start_secs_rem = start_secs % 60;
    let end_mins = end_secs / 60;
    let end_secs_rem = end_secs % 60;
    let sel_mins = selection_duration / 60;
    let sel_secs_rem = selection_duration % 60;
    let selection_duration_f32 = selection_duration as f32;

    // Get timestamp
    let timestamp = get_timestamp();

    if let Some(frontmatter_end) = original_content.find("\n---\n") {
        // Has YAML frontmatter - parse to extract tags
        let yaml_section = &original_content[..frontmatter_end];
        let markdown_section = &original_content[frontmatter_end + 5..]; // Skip "\n---\n"

        // Parse original YAML to get tags
        let mut tags = vec!["excerpt".to_string()];
        if let Ok(yaml_value) = serde_yaml::from_str::<serde_yaml::Value>(yaml_section)
            && let Some(original_tags) = yaml_value.get("tags").and_then(|v| v.as_sequence())
        {
            for tag in original_tags {
                if let Some(tag_str) = tag.as_str()
                    && tag_str != "excerpt"
                    && !tags.contains(&tag_str.to_string())
                {
                    tags.push(tag_str.to_string());
                }
            }
        }

        let tags_yaml = format_tags(&tags);

        return Ok(format!(
            r#"---
file: "{dest_filename}"
path: "{dest_dir}"
title: "{dest_filename}"
description: "Excerpt from {source_filename}"
duration: {selection_duration_f32:.2}
tags: {tags_yaml}
source_file: "{source_path}"
source_time_start: {start_mins}:{start_secs_rem:02}
source_time_end: {end_mins}:{end_secs_rem:02}
source_duration: {sel_mins}:{sel_secs_rem:02}
extracted_at: {timestamp}
extraction_type: "selection"
---

{markdown_section}"#
        ));
    }

    // No YAML frontmatter - create new with fallback tags
    let tags = if let Some(fallback) = tags_fallback {
        let mut all_tags = vec!["excerpt".to_string()];
        for tag in fallback {
            if tag != "excerpt" && !all_tags.contains(tag) {
                all_tags.push(tag.clone());
            }
        }
        all_tags
    } else {
        vec!["excerpt".to_string()]
    };

    let tags_yaml = format_tags(&tags);

    Ok(format!(
        r#"---
file: "{dest_filename}"
path: "{dest_dir}"
title: "{dest_filename}"
description: "Excerpt from {source_filename}"
duration: {selection_duration_f32:.2}
tags: {tags_yaml}
source_file: "{source_path}"
source_time_start: {start_mins}:{start_secs_rem:02}
source_time_end: {end_mins}:{end_secs_rem:02}
source_duration: {sel_mins}:{sel_secs_rem:02}
extracted_at: {timestamp}
extraction_type: "selection"
---

# {dest_filename}

**Excerpt from: {source_filename}**

Time range: {start_mins}:{start_secs_rem:02} - {end_mins}:{end_secs_rem:02} (duration: {sel_mins}:{sel_secs_rem:02})

## Notes

Add your notes and tags here to document this excerpt.
"#
    ))
}

/// Format tags as YAML array
fn format_tags(tags: &[String]) -> String {
    if tags.is_empty() {
        "[]".to_string()
    } else {
        format!(
            "[{}]",
            tags.iter()
                .map(|t| format!("\"{t}\""))
                .collect::<Vec<_>>()
                .join(", ")
        )
    }
}

/// Get current timestamp in ISO 8601 format using the chrono crate.
///
/// This provides cross-platform timestamp generation without relying on
/// shell commands that may not be available on all systems (e.g., Windows).
fn get_timestamp() -> String {
    use chrono::Utc;
    Utc::now().to_rfc3339()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_sidecar_path() {
        let audio = PathBuf::from("/music/track.wav");
        let sidecar = get_sidecar_path(&audio);
        assert_eq!(sidecar, PathBuf::from("/music/track.wav.md"));
    }

    #[test]
    fn test_get_sidecar_path_flac() {
        let audio = PathBuf::from("./samples/drum_loop.flac");
        let sidecar = get_sidecar_path(&audio);
        assert_eq!(sidecar, PathBuf::from("./samples/drum_loop.flac.md"));
    }

    #[test]
    fn test_format_tags_empty() {
        assert_eq!(format_tags(&[]), "[]");
    }

    #[test]
    fn test_format_tags_single() {
        assert_eq!(format_tags(&["drum".to_string()]), "[\"drum\"]");
    }

    #[test]
    fn test_format_tags_multiple() {
        assert_eq!(
            format_tags(&["drum".to_string(), "loop".to_string()]),
            "[\"drum\", \"loop\"]"
        );
    }
}
