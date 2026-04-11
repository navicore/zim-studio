# Architecture

ZIM Studio is a Rust CLI (`zim`) with two main capabilities: audio project management and a terminal audio player. The player is behind an optional `player` feature flag.

## Binary Entry Point

`src/main.rs` — Defines the CLI via `clap` with subcommands: `init`, `config`, `new`, `update`, `lint`, `index`, `sync`, `add`, `tag`, `play`, `completions`. Each subcommand dispatches to a handler in `src/cli/`.

## Module Layout

```
src/
  main.rs              # CLI definition & dispatch
  lib.rs               # Public library exports (config, constants, utils, wav_metadata, zimignore; media behind feature flag)

  cli/                 # Subcommand handlers
    init.rs            # Interactive first-run setup (~/.config/zim/config.toml)
    config.rs          # View/set/edit config
    new.rs             # Scaffold new project directories
    update.rs          # Generate/update sidecar .md files for audio files
    lint.rs            # Validate YAML frontmatter in sidecars
    index.rs           # Generate index.yml from sidecar metadata
    sync.rs            # Re-sync sidecar technical metadata with audio files
    add.rs             # Add tags to existing sidecar files
    tag.rs             # Embed/read WAV INFO LIST metadata (UUID, lineage)
    play.rs            # Launch the TUI player

  config.rs            # Config file parsing (~/.config/zim/config.toml)
  constants.rs         # Shared constants
  zimignore.rs         # .zimignore file handling (like .gitignore for audio)
  wav_metadata.rs      # WAV INFO LIST chunk read/write for embedded metadata
  templates/           # Templates for scaffolded project files
  project/             # Project directory structure logic
  media/               # Audio file metadata extraction (behind player feature)
  utils/
    sidecar.rs         # Sidecar .md file generation and parsing
    validation.rs      # YAML frontmatter validation
    parallel_scan.rs   # Parallel directory scanning (rayon)
    progress.rs        # Progress bar display (indicatif)
    project.rs         # Project path utilities

  player/              # TUI audio player (behind "player" feature flag)
    app.rs             # Player application state and event loop
    ui.rs              # Ratatui UI rendering
    audio.rs           # Audio playback via rodio
    browser.rs         # Telescope-style fuzzy file browser
    waveform.rs        # Oscilloscope / braille waveform rendering
    timeline_waveform.rs # Waveform on the progress timeline
    save_dialog.rs     # Save dialog state
    save_dialog_ui.rs  # Save dialog rendering
    mixed_source.rs    # Multi-file mixing (up to 3 sources with gain control)
    telemetry.rs       # Optional JSON telemetry logging
```

## Key Data Flows

**Sidecar generation** (`zim update`): Scans directory for `.wav`/`.flac` files (parallel via rayon), extracts audio metadata, infers tags from filename patterns (configurable in config.toml), generates markdown sidecar files with YAML frontmatter. Also auto-tags untagged WAV files with embedded UUID metadata.

**WAV metadata embedding** (`zim tag`): Reads/writes UUID, lineage, and project info into WAV INFO LIST chunks. Supports derive chains for tracking file provenance across DAW workflows.

**Player** (`zim play`): Ratatui TUI with crossterm backend. Audio playback via rodio. Supports single-file playback with mark/loop/save, multi-file mixing with per-track gain, and a fuzzy file browser that searches sidecar markdown content.

## Feature Flags

- `default = ["player"]` — includes the full TUI player and all audio dependencies
- Without `player` — CLI-only mode for scaffolding and metadata management

## External Dependencies

- **clap** — CLI parsing
- **ratatui/crossterm** — TUI (player feature)
- **rodio/hound/claxon** — Audio playback and WAV/FLAC decoding (player feature)
- **serde/serde_yaml/toml** — Config and metadata serialization
- **rayon** — Parallel file scanning
- **uuid/md5** — File identity and fingerprinting
- **fuzzy-matcher** — Browser search (player feature)

## Configuration

Global config lives at `~/.config/zim/config.toml`. Stores root directory, default artist, tag mappings, and project name normalization preferences. Per-project `.zimignore` controls which files are excluded from sidecar generation.

## Tests

Integration tests in `tests/config_test.rs`. The `justfile` provides `just ci` to run fmt-check, clippy, test, and build in sequence — the same recipe GitHub Actions runs.
