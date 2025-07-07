pub struct SidecarMetadata<'a> {
    pub file_name: &'a str,
    pub file_path: &'a str,
    pub sample_rate: u32,
    pub channels: u16,
    pub bits_per_sample: u16,
    pub duration_seconds: Option<f64>,
    pub file_size: u64,
    pub modified: Option<&'a str>,
}

pub fn generate_minimal_sidecar_with_fs_metadata(
    file_name: &str,
    file_path: &str,
    file_size: u64,
    modified: Option<&str>,
) -> String {
    let modified_str = modified.unwrap_or("unknown");

    format!(
        r#"---
file: "{file_name}"
path: "{file_path}"
title: ""
description: ""
file_size: {file_size}
modified: "{modified_str}"
tags: []
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

    format!(
        r#"---
file: "{}"
path: "{}"
title: ""
description: ""
duration: {duration_str}
sample_rate: {}
channels: {}
bit_depth: {}
file_size: {}
modified: "{modified_str}"
tags: []
art: []
---

# Notes

[Add notes about this file here]
"#,
        metadata.file_name,
        metadata.file_path,
        metadata.sample_rate,
        metadata.channels,
        metadata.bits_per_sample,
        metadata.file_size,
    )
}
