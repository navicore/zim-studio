pub struct SidecarMetadata<'a> {
    pub file_name: &'a str,
    pub file_path: &'a str,
    pub title: &'a str,
    pub description: &'a str,
    pub tags: &'a [String],
    pub sample_rate: u32,
    pub channels: u16,
    pub bits_per_sample: u16,
    pub duration_seconds: Option<f64>,
    pub file_size: u64,
    pub modified: Option<&'a str>,
    pub project: Option<&'a str>,
    pub uuid: Option<&'a str>,
}

#[allow(clippy::too_many_arguments)]
pub fn generate_minimal_sidecar_with_fs_metadata(
    file_name: &str,
    file_path: &str,
    title: &str,
    description: &str,
    tags: &[String],
    file_size: u64,
    modified: Option<&str>,
    project: Option<&str>,
    uuid: Option<&str>,
) -> String {
    let modified_str = modified.unwrap_or("unknown");
    let project_str = project.unwrap_or("unknown");

    // Format tags as YAML array
    let tags_str = if tags.is_empty() {
        "[]".to_string()
    } else {
        format!("[\"{}\"]", tags.join("\", \""))
    };

    // Include UUID if available
    let uuid_line = if let Some(id) = uuid {
        format!("uuid: \"{id}\"\n")
    } else {
        String::new()
    };

    format!(
        r#"---
file: "{file_name}"
path: "{file_path}"
project: "{project_str}"
{uuid_line}title: "{title}"
description: "{description}"
file_size: {file_size}
modified: "{modified_str}"
tags: {tags_str}
art: []
---

# Notes

[Add notes about this file here]
"#
    )
}

pub fn generate_audio_sidecar_with_metadata(metadata: &SidecarMetadata) -> String {
    let duration_str = metadata
        .duration_seconds
        .map(|d| format!("{d:.2}"))
        .unwrap_or_else(|| "unknown".to_string());

    let modified_str = metadata.modified.unwrap_or("unknown");
    let project_str = metadata.project.unwrap_or("unknown");

    // Format tags as YAML array
    let tags_str = if metadata.tags.is_empty() {
        "[]".to_string()
    } else {
        format!("[\"{}\"]", metadata.tags.join("\", \""))
    };

    // Include UUID if available
    let uuid_line = if let Some(id) = metadata.uuid {
        format!("uuid: \"{id}\"\n")
    } else {
        String::new()
    };

    format!(
        r#"---
file: "{}"
path: "{}"
project: "{project_str}"
{uuid_line}title: "{}"
description: "{}"
duration: {duration_str}
sample_rate: {}
channels: {}
bit_depth: {}
file_size: {}
modified: "{modified_str}"
tags: {tags_str}
art: []
---

# Notes

[Add notes about this file here]
"#,
        metadata.file_name,
        metadata.file_path,
        metadata.title,
        metadata.description,
        metadata.sample_rate,
        metadata.channels,
        metadata.bits_per_sample,
        metadata.file_size,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_minimal_sidecar_with_fs_metadata() {
        let content = generate_minimal_sidecar_with_fs_metadata(
            "test.mp3",
            "/path/to/test.mp3",
            "test",
            "A source for my-project",
            &["source".to_string()],
            1234567,
            Some("2024-01-15 10:30:00 UTC"),
            Some("my-project"),
            None, // No UUID for this test
        );

        assert!(content.contains("file: \"test.mp3\""));
        assert!(content.contains("path: \"/path/to/test.mp3\""));
        assert!(content.contains("project: \"my-project\""));
        assert!(content.contains("title: \"test\""));
        assert!(content.contains("description: \"A source for my-project\""));
        assert!(content.contains("tags: [\"source\"]"));
        assert!(content.contains("file_size: 1234567"));
        assert!(content.contains("modified: \"2024-01-15 10:30:00 UTC\""));
        assert!(content.contains("art: []"));
        assert!(content.contains("# Notes"));
    }

    #[test]
    fn test_generate_minimal_sidecar_without_modified() {
        let content = generate_minimal_sidecar_with_fs_metadata(
            "test.aiff",
            "./test.aiff",
            "test",
            "",
            &[],
            999,
            None,
            None,
            None, // No UUID for this test
        );

        assert!(content.contains("modified: \"unknown\""));
        assert!(content.contains("project: \"unknown\""));
    }

    #[test]
    fn test_generate_audio_sidecar_with_metadata() {
        let metadata = SidecarMetadata {
            file_name: "audio.wav",
            file_path: "/music/audio.wav",
            title: "audio",
            description: "A mix for awesome-project",
            tags: &["mix".to_string()],
            sample_rate: 44100,
            channels: 2,
            bits_per_sample: 16,
            duration_seconds: Some(123.45),
            file_size: 5432100,
            modified: Some("2024-01-15 10:30:00 UTC"),
            project: Some("awesome-project"),
            uuid: Some("test-uuid-12345"),
        };

        let content = generate_audio_sidecar_with_metadata(&metadata);

        assert!(content.contains("file: \"audio.wav\""));
        assert!(content.contains("path: \"/music/audio.wav\""));
        assert!(content.contains("project: \"awesome-project\""));
        assert!(content.contains("uuid: \"test-uuid-12345\""));
        assert!(content.contains("title: \"audio\""));
        assert!(content.contains("description: \"A mix for awesome-project\""));
        assert!(content.contains("tags: [\"mix\"]"));
        assert!(content.contains("duration: 123.45"));
        assert!(content.contains("sample_rate: 44100"));
        assert!(content.contains("channels: 2"));
        assert!(content.contains("bit_depth: 16"));
        assert!(content.contains("file_size: 5432100"));
        assert!(content.contains("modified: \"2024-01-15 10:30:00 UTC\""));
    }

    #[test]
    fn test_generate_audio_sidecar_without_duration() {
        let metadata = SidecarMetadata {
            file_name: "audio.flac",
            file_path: "./audio.flac",
            title: "audio",
            description: "",
            tags: &[],
            sample_rate: 48000,
            channels: 1,
            bits_per_sample: 24,
            duration_seconds: None,
            file_size: 1000000,
            modified: None,
            project: None,
            uuid: None,
        };

        let content = generate_audio_sidecar_with_metadata(&metadata);

        assert!(content.contains("duration: unknown"));
        assert!(content.contains("modified: \"unknown\""));
        assert!(content.contains("project: \"unknown\""));
    }

    #[test]
    fn test_yaml_frontmatter_format() {
        let content = generate_minimal_sidecar_with_fs_metadata(
            "test.mp3",
            "/test.mp3",
            "test",
            "",
            &[],
            1000,
            None,
            None,
            None, // No UUID for this test
        );

        // Check YAML frontmatter delimiters
        assert!(content.starts_with("---\n"));
        assert!(content.contains("\n---\n"));

        // Ensure proper structure
        let parts: Vec<&str> = content.split("---").collect();
        assert_eq!(parts.len(), 3); // Empty start, YAML content, markdown content
    }
}
