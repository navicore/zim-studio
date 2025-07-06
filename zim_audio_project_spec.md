# ZIM: Terminal-Based Audio Project Scaffold and Metadata System

*A Zettelkasten Information system for Music*

## Overview

**ZIM** is a terminal-native tool for audio creators who manage projects with
structured folders, DAW sessions, and Zettelkasten-style Markdown notes. It
provides a reproducible way to initialize and manage audio projects using a
global configuration, sidecar metadata files, and extensible workflows.

---

## Goals

- Scaffold audio projects with consistent folder structures.
- Use Markdown (`.md`) sidecar files for human-readable and machine-parsable metadata.
- Respect existing workflows that span multiple DAWs (e.g., Ableton, Reaper).
- Avoid tracking large binary files in Git while still versioning creative metadata.
- Fit naturally with Zettelkasten note-taking and Telescope-based Neovim workflows.

---

## Command Line Interface (CLI)

### `zim new [project-name]`

Create a new audio project scaffold.

If `[project-name]` is omitted, ZIM auto-generates a unique name using today's
date and an incremented integer (e.g., `20250706-001`).

#### Generated Structure:

```
~/MusicProjects/
└── swampy-groove/
    ├── swampy-groove.md
    ├── .gitignore
    ├── sources/
    │   └── README.md
    ├── edits/
    │   └── README.md
    ├── processed/
    │   └── README.md
    ├── mixes/
    │   └── README.md
    ├── masters/
    │   └── README.md
    └── project/
        ├── ableton/
        │   └── (e.g., swampy.als)
        └── reaper/
            └── (e.g., swampy.rpp)
```

---

### `zim update`

Scans the project directory recursively. For every recognized media file:

- If a sidecar `.md` file does not exist, one is generated.
- Sidecar includes YAML frontmatter with metadata like:
  - `file`, `path`, `duration`, `sample_rate`, `channels`, `bit_depth`, `hash`
- Markdown body includes a placeholder for notes.

Example sidecar (`processed/gtr_fx.wav.md`):

```markdown
---
file: "gtr_fx.wav"
path: "processed/gtr_fx.wav"
hash: "a3b7c8..."
duration: 12.42
sample_rate: 48000
channels: 2
bit_depth: 24
tags: []
---

# Notes

Add verb and saturation starting at 3s. Double-check transient smear around 9s.
```

---

## Global Configuration

Stored at `~/.config/zim/config.toml` (or `$XDG_CONFIG_HOME/zim/config.toml`).

### Example:

```toml
root_dir = "~/MusicProjects"
default_artist = "navicore"
default_folders = ["sources", "edits", "processed", "mixes", "masters", "project"]
default_gitignore = ["*.wav", "*.flac", "*.aiff", "*.als~", "project/*/temp/"]
include_readmes = true
```

---

## Folder Semantics

| Folder       | Purpose                                       |
| ------------ | --------------------------------------------- |
| `sources/`   | Raw recordings (e.g., from iPad or field mic) |
| `edits/`     | Chopped/trimmed versions of raw files         |
| `processed/` | EQ’d, compressed, FX-enhanced versions        |
| `mixes/`     | Combined track renders (pre-master)           |
| `masters/`   | Finalized, polished versions                  |
| `project/`   | DAW-specific session subfolders               |

## Git Strategy

- Git is used to version `.md`, `.json`, `.yaml`, and `.gitignore`.
- Large binary files (media) are ignored.
- Example `.gitignore`:

```gitignore
*.wav
*.flac
*.aiff
*.als~
project/*/temp/
```

---

## Naming Philosophy

- Keep media filenames short and stable (e.g., `gtr.wav`, `loop1.wav`).
- Use folder structure to express stage (`sources/`, `processed/`, etc.).
- Metadata lives in sidecar `.md` files.

---

## Extensibility Ideas

- `zim tag`: Add or update tags in a media file’s sidecar.
- `zim list recent`: Show recently updated or created projects.
- `zim status`: Scan for unversioned sidecars or missing metadata.
- `zim export`: Convert project metadata to JSON or publishable README.

---

## Project Layout for Implementation (Rust)

```
src/
├── main.rs
├── cli.rs
├── config.rs
├── project/
│   ├── mod.rs
│   ├── scaffold.rs
│   └── update.rs
├── media/
│   ├── mod.rs
│   └── metadata.rs
├── templates/
│   ├── mod.rs
│   └── markdown.rs
├── utils/
│   ├── mod.rs
│   └── hash.rs
```

---

## Naming Justification

`` is inspired by:

- `z` for Zettelkasten
- `m` for music
- Terminal-native aesthetics (`vim`, `tmux`, `fzf`)
- Short, fast to type, expressive in CLI context

---

## Example Commands

```bash
zim new swampy-groove
zim new                 # auto-generates name like 20250706-001
zim update              # generates sidecars
zim config view         # prints current config
zim config set root_dir ~/MyMusicProjects
```

---


