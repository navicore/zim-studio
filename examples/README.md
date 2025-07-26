# ZIM Examples

This directory contains example sidecar files showing the YAML + Markdown format that ZIM uses.

## Files

- [`minimal-sidecar.md`](minimal-sidecar.md) - A freshly generated sidecar with minimal metadata
- [`sidecar-example.md`](sidecar-example.md) - A fully populated sidecar showing all features:
  - User-provided title and description
  - Audio technical metadata (from FLAC/WAV files)
  - File system metadata
  - Tags for organization
  - Visual art references with purposes
  - Extensive markdown notes including recording details, mix notes, TODOs, and links
- [`extracted-selection-example.md`](extracted-selection-example.md) - **New in v1.0.0**: Shows provenance tracking for extracted audio selections:
  - Automatic duration update for the extracted section
  - Source file path and time range tracking
  - ISO 8601 extraction timestamp
  - Maintains original metadata context

## YAML Fields

### Required Fields (automatically generated)
- `file`: Filename
- `path`: Relative path within project
- `file_size`: Size in bytes
- `modified`: Last modification timestamp

### Optional Fields (user-editable)
- `title`: Track/file title
- `description`: Brief description
- `tags`: Array of string tags
- `art`: Array of visual references, each with:
  - `path`: Path to visual file
  - `description`: What it is
  - `purpose`: One of `inspiration`, `cover_art`, or `other`

### Audio-specific Fields (auto-extracted from FLAC/WAV/AIFF)
- `duration`: Length in seconds (updated automatically for extracted selections)
- `sample_rate`: Hz (e.g., 44100, 48000)
- `channels`: Number of channels
- `bit_depth`: Bits per sample

### Provenance Fields (added automatically for extracted selections)
- `source_file`: Absolute path to the original audio file
- `source_time_start`: Start time in MM:SS format within the source
- `source_time_end`: End time in MM:SS format within the source  
- `source_duration`: Length of extracted selection in MM:SS format
- `extracted_at`: ISO 8601 timestamp when the selection was extracted
- `extraction_type`: Always "selection" for audio extracts

## Validation

Run `zim lint` to validate all YAML frontmatter in your project. This ensures your metadata is properly formatted before using it in scripts or automation.