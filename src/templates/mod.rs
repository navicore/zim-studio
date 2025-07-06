// Reserved for future use when we extract media metadata
#[allow(dead_code)]
pub fn generate_media_sidecar(metadata: &crate::media::MediaMetadata) -> String {
    let yaml = serde_yaml::to_string(metadata).unwrap_or_default();

    format!(
        r#"---
{yaml}---

# Notes

[Add notes about this file here]

## Technical Details

[Processing notes, effects used, etc.]

## Creative Notes

[Musical ideas, emotions, context]

## Visual References

[Any visual associations or artwork specific to this track]
"#
    )
}

pub fn generate_minimal_sidecar(file_name: &str, file_path: &str) -> String {
    format!(
        r#"---
file: "{file_name}"
path: "{file_path}"
tags: []
art: []
---

# Notes

[Add notes about this file here]
"#
    )
}
