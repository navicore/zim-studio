[![Dependabot Updates](https://github.com/navicore/zim-studio/actions/workflows/dependabot/dependabot-updates/badge.svg)](https://github.com/navicore/zim-studio/actions/workflows/dependabot/dependabot-updates)
[![CI](https://github.com/navicore/zim-studio/actions/workflows/ci.yml/badge.svg)](https://github.com/navicore/zim-studio/actions/workflows/ci.yml)
[![Release with Auto Version (PAT)](https://github.com/navicore/zim-studio/actions/workflows/release.yml/badge.svg)](https://github.com/navicore/zim-studio/actions/workflows/release.yml)

# ZIM Studio - Terminal-Based Audio Tools

*A [Zettelkasten](https://en.wikipedia.org/wiki/Zettelkasten) Information System for Music Production with Integrated Audio Player*

_Implemented in [Rust](https://www.rust-lang.org) and [Ratatui](https://ratatui.rs) with optional [nvim plugin](https://github.com/navicore/zim-studio-nvim) support._

![IMAGE: Screenshot of main player interface](docs/zim_play.png)

![IMAGE: Screenshot of browser interface](docs/zim_prev.png) 

ZIM Studio provides three main functions:
  1. **Project Management**: Initialize a project structure of directories and placeholder README.md files
  2. **Metadata System**: Generate searchable sidecar files for all audio media
  3. **Audio Player**: Sample browsing, auditioning, and editing with TUI interface

## ✨ More...

- **Supported Audio Formats**: `*.flac` and `*.wav` (so far)
- **Enhanced Navigation**: Shift+Arrow keys for 20% jumps through long recordings
- **Smart Sidecar Cloning**: When saving selections, automatically clones source metadata with:
  - Updated duration for the extracted selection
  - Provenance tracking (source file, time ranges)

The sidecar format is YAML embedded in markdown, providing both structured metadata
and free-form notes. The YAML contains facts about the track while the markdown
enables long-form notes, links to inspiration, TODO checklists, etc.

See [example sidecar file](examples/sidecar-example.md) for a complete example.

The motivation for creating `zim` is twofold:

  1. Maintain a workflow where DAWs are guests in my workflow rather than the
     other way around.
  2. I use [neovim](https://neovim.io/) and [telescope](https://github.com/nvim-telescope/telescope.nvim) for my
     personal note taking and rely on no proprietary company to enable my
     ability to work in software ... I wanted the same detailed note taking
     independent of any vendor for my music making.  I use proprietary DAWs but
     am not ok living in DAWs or any vendor's closed system.

## Installation

```bash
# Install with full features (includes audio player)
cargo install zim-studio

# Install minimal version without audio player (scaffold and metadata only)
cargo install zim-studio --no-default-features
```

If you use neovim (untested with regular vim) you may want to try the
[nvim plugin](https://github.com/navicore/zim-studio-nvim).

![IMAGE: Screenshot of main player interface run from nvim oil](docs/nvim_zim.png)

## Quick Start

```bash
# Initialize ZIM with your music projects directory
zim init ~/Music/Projects

# Create a new project
zim new "My Greatest Hits"
# Creates: ~/Music/Projects/my_greatest_hits/

# Navigate to your project and add some audio files
cd ~/Music/Projects/my_greatest_hits
cp ~/Desktop/track1.flac masters/
cp ~/Desktop/track2.wav masters/

# Generate sidecar metadata files
zim update .
# Creates: masters/track1.flac.md, masters/track2.wav.md

# Edit the generated sidecar files to add your notes
$EDITOR masters/track1.flac.md

# Validate all YAML frontmatter
zim lint .

# View/edit global configuration
zim config view
zim config edit
```

## Project Structure

When you create a new project with `zim new`, it generates:

```
my_greatest_hits/
├── .gitignore          # Ignores audio/video files
├── README.md           # Project overview
├── masters/            # Final mastered tracks
├── mixes/              # Mix versions
├── sources/            # Raw recordings, samples
├── edits/              # Edited/comped audio
├── bounced/            # Bounced/rendered audio (stems, etc)
└── project/            # DAW project files
    ├── live/           # Ableton Live
    ├── reaper/         # Reaper
    ├── bitwig/         # Bitwig Studio
    └── renoise/        # Renoise
```

## Sidecar Files

`zim update` generates a `.md` sidecar for each audio file with:
- **YAML frontmatter**: Structured metadata (technical specs, tags, etc.)
- **Markdown body**: Free-form notes, ideas, TODO lists
- **Automatic tag inference**: Tags are automatically added based on filename patterns (e.g., files with "ES-9" get tagged "eurorack", "drum" files get tagged "drums")

The YAML is designed to be both human-editable and scriptable for automation.
See the [example sidecar](examples/sidecar-example.md) for what this looks like in practice.

### Automatic Tag Inference

ZIM automatically infers tags from filenames using configurable pattern mappings. Default mappings include:
- Hardware: `ES-9` → `eurorack`, `modular` → `eurorack`
- Instruments: `drum` → `drums`, `bass` → `bass`, `synth` → `synth`
- Structure: `loop` → `loop`, `kick` → `drums`, `snare` → `drums`
- Vocals: `vocal` → `vocals`, `vox` → `vocals`
- DAWs: `ableton` → `ableton-live`, `reaper` → `reaper`

To customize tag mappings, edit `~/.config/zim/config.toml`:
```toml
[tag_mappings]
"ES-9" = "eurorack"
"my-pattern" = "my-tag"
"field-rec" = "field-recording"
```

**Note**: If you have an existing config file, the default tag mappings will still work automatically - no migration needed!

## WAV Metadata Tagging

ZIM can embed metadata directly into WAV files using INFO LIST chunks. This metadata includes UUIDs for unique identification and lineage tracking across your DAW workflows.

### Tag Commands

#### `zim tag add` - Tag a copy of the file
Creates a new WAV file with `_tagged` suffix containing embedded metadata. The original file remains unchanged.

```bash
# Tag a WAV file (creates kick_tagged.wav)
zim tag add kick.wav

# Specify project explicitly (otherwise auto-detected from .zimignore)
zim tag add kick.wav --project "my-album"
```

#### `zim tag edit` - Tag in-place
Updates the original WAV file directly with embedded metadata. Creates a backup in `/tmp` by default.

```bash
# Update metadata in the original file
zim tag edit kick.wav

# Skip backup creation (use with caution)
zim tag edit kick.wav --no-backup
```

#### `zim tag derive` - Create derived file with lineage
Creates a new WAV file that tracks its relationship to the source file. Perfect for tracking exports, bounces, and transformations.

```bash
# Create a derived file (e.g., after processing in a DAW)
zim tag derive original.wav processed.wav --transform "eq+compress"

# Common transform types: excerpt, mix, master, bounce, process
zim tag derive full_take.wav intro_only.wav --transform "excerpt"
```

#### `zim tag info` - Read embedded metadata
Displays any ZIM metadata embedded in a WAV file.

```bash
# Check if a WAV file has metadata
zim tag info some_file.wav
```

### Metadata Fields

Each tagged WAV file contains:
- **UUID**: Unique identifier for the file
- **Parent UUID**: Link to source file (for derived files)
- **Project**: Associated project name
- **Generation**: How many transformations from the original (0 = original)
- **Transform**: Type of processing applied (for derived files)
- **Audio MD5**: Fingerprint of the audio data
- **First Seen**: ISO 8601 timestamp when first tagged
- **Original Path**: Where the file was first tagged

### Integration with `zim update`

The `zim update` command automatically tags any untagged WAV files it encounters, embedding metadata directly into the files. The UUID is also included in the generated markdown sidecar files for cross-referencing.

```bash
# Recursively process directory, auto-tagging WAV files
zim update .
```

### Use Cases

1. **Import Tracking**: When importing WAV files into a DAW, they retain their identity
2. **Export Lineage**: Track which files were derived from which sources
3. **Collaboration**: Share WAV files that carry their own provenance
4. **Deduplication**: Identify the same audio even if files are renamed
5. **Project Organization**: Find all files belonging to a specific project

**Note**: If you have an existing config file, the default tag mappings will still work automatically - no migration needed!

## Shell Completions

Add to your shell configuration:

```bash
# Bash (~/.bashrc)
source <(zim completions bash)

# Zsh (~/.zshrc)
source <(zim completions zsh)

# Fish (~/.config/fish/config.fish)
zim completions fish | source

# PowerShell ($PROFILE)
zim completions powershell | Out-String | Invoke-Expression
```

## Development

```bash
# Run all checks locally (matches CI)
make ci

# Individual commands
make fmt        # Format code
make clippy     # Run lints
make test       # Run tests
make check      # Check compilation
```

## Audio Player User Guide

The optional audio player provides a fast, keyboard-driven interface for browsing, auditioning, and editing audio samples directly from the terminal.

### Launching the Player

```bash
# Launch with no file (opens browser)
zim player

# Launch with a specific audio file
zim player path/to/audio.wav
```

### Main Interface

![IMAGE: Screenshot of main player interface with oscilloscope and controls](docs/player_main.png)

The player interface consists of:
- **Title Bar**: Shows "🎵 ZIM Player"
- **File Info & LEDs**: Current file name and stereo level indicators
- **Progress Bar**: Playback position with mark in/out indicators
- **Oscilloscope**: Real-time waveform visualization (when window is tall enough)
- **Control Hints**: Two rows of keyboard shortcuts

### Keyboard Controls

#### Playback Controls
- `[space]` - Play/Pause toggle
- `[←]` - Seek backward 5 seconds
- `[→]` - Seek forward 5 seconds
- `[Shift+←]` - Jump backward 20% (great for long recordings)
- `[Shift+→]` - Jump forward 20% (great for long recordings)

#### Mark & Loop Controls
- `[i]` - Set mark in at current position
- `[o]` - Set mark out at current position
- `[x]` - Clear all marks
- `[l]` - Toggle loop playback of marked selection

#### File Operations
- `[/]` - Open file browser
- `[e]` - Edit sidecar metadata in external editor ($EDITOR)
- `[s]` - Save/export (full file or marked selection)
- `[q]` - Quit player

### File Browser

![IMAGE: Screenshot of telescope-style file browser](docs/player_file_browser.png)

The built-in file browser searches through your audio files using their sidecar `.md` metadata:

1. Press `/` to open the browser
2. Use `[↑/↓]` or `[j/k]` to navigate files
3. Press `/` again to show search overlay
4. Type to search - it searches within the markdown content of sidecar files
5. Press `[Enter]` or `[Esc]` to hide search and return to file list
6. Press `[Enter]` on a file to load it
7. Press `[Esc]` to close browser

**Note**: The browser displays audio files but searches their `.md` sidecar content. For example, if you have `kick.wav` with `kick.wav.md` containing "punchy 808 style", searching for "808" will find this file.

### Mark In/Out & Looping

![IMAGE: Screenshot showing marks on progress bar](docs/player_marks.png)

The mark feature lets you select a portion of the audio:

1. Play the file and press `[i]` at the desired start point
2. Press `[o]` at the desired end point
3. The selection appears highlighted on the progress bar
4. Press `[l]` to loop the selection continuously
5. The time display shows selection duration in brackets: `[3.5s]`

### Save Dialog

![IMAGE: Screenshot of save dialog with directory browser](docs/player_save.png)

When saving (`[s]`), the save dialog provides:

- **Directory Browser**: Navigate folders with `[↑/↓]` and `[Enter]`
- **Filename Field**: Editable with smart naming for edits
- **Tab Navigation**: Use `[Tab]` to switch between directory list and filename
- **Smart Extensions**:
  - Selections always save as `.wav` (even from FLAC sources)
  - Full file saves preserve original format

Example auto-generated filenames:
- First edit: `original_edit.wav`
- Subsequent edits: `original_edit_2.wav`, `original_edit_3.wav`, etc.

### LED Level Indicators

[IMAGE: Close-up of LED indicators showing different levels]

The stereo LED indicators show real-time audio levels:
- **L (Left)**: Green LEDs - dim → medium → bright → red (clipping)
- **R (Right)**: Orange LEDs - dim → medium → bright → red (clipping)
- **Symbols**: ○ (off/quiet) → ◐ (medium) → ● (loud)

### Supported Formats

- **WAV**: 8, 16, 24, and 32-bit
- **FLAC**: All bit depths (converted to 16-bit WAV when saving selections)
- ~~**AIFF**: All bit depths with intelligent sample rate detection~~ WIP

### Tips & Workflow

1. **Quick Sample Chopping**: Load file → mark in/out → save = done
2. **Preview Before Save**: Use `[l]` to loop your selection before saving
3. **Rapid Browsing**: The browser remembers your search, making it fast to audition similar samples
4. **Long Recording Navigation**: Use Shift+Arrow for 20% jumps to quickly navigate through long recordings
5. **Metadata Preservation**: When saving selections, source metadata is automatically cloned with updated duration and provenance tracking
6. **No Mouse Needed**: Everything is keyboard-driven for speed
7. **Mix Multiple Files**: Audition up to 3 files simultaneously with individual gain control:
   ```bash
   # Mix with equal volume
   zim play drums.wav bass.wav vocals.wav
   
   # Mix with custom gains (0.0-2.0 range)
   zim play drums.wav bass.wav vocals.wav --gains 0.8,1.2,0.6
   ```
   See [mixing guide](docs/mixing-guide.md) for detailed examples

### Troubleshooting

- **No Audio**: Check system audio output settings
- **Browser Not Finding Files**: Ensure `.md` sidecar files exist (run `zim update`)
- **Visual Glitches**: Resize terminal window or restart player

## License

MIT
