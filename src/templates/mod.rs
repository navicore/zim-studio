use crate::media::{MediaMetadata, ArtEntry};

pub fn generate_media_sidecar(metadata: &MediaMetadata) -> String {
    let yaml = serde_yaml::to_string(metadata).unwrap_or_default();
    
    format!(
        r#"---
{}---

# Notes

[Add notes about this file here]

## Technical Details

[Processing notes, effects used, etc.]

## Creative Notes

[Musical ideas, emotions, context]

## Visual References

[Any visual associations or artwork specific to this track]
"#,
        yaml
    )
}

pub fn generate_minimal_sidecar(file_name: &str, file_path: &str) -> String {
    format!(
        r#"---
file: "{}"
path: "{}"
tags: []
art: []
---

# Notes

[Add notes about this file here]
"#,
        file_name,
        file_path
    )
}