[![Dependabot Updates](https://github.com/navicore/zim-studio/actions/workflows/dependabot/dependabot-updates/badge.svg)](https://github.com/navicore/zim-studio/actions/workflows/dependabot/dependabot-updates)
[![CI](https://github.com/navicore/zim-studio/actions/workflows/ci.yml/badge.svg)](https://github.com/navicore/zim-studio/actions/workflows/ci.yml)
[![Release with Auto Version (PAT)](https://github.com/navicore/zim-studio/actions/workflows/release.yml/badge.svg)](https://github.com/navicore/zim-studio/actions/workflows/release.yml)

# ZIM - Terminal-Based Audio Project Scaffold and Metadata System

*A [Zettelkasten](https://en.wikipedia.org/wiki/Zettelkasten) Information System for Music*

The main two functions of this tool are:
  1. Initialize a project structure of directories and placeholder README.md files
       for each new project as needed
  2. Generate sidecar Metadata files for all audio media

The sidecar format is yaml embedded in markdown.  It isn't as bad as it sounds.
The `yaml` has facts about the track or project and the `markdown` lets the user
create long-form notes about the work, links to inspiration, `TODO` checklists,
etc...

See [example sidecar file](examples/sidecar-example.md) for a complete example.

The motivation for creating `zim` is twofold:

  1. Maintain a workflow where DAWs are guests in my workflow rather than the
     other way around.
  2. I use [neovim](https://neovim.io/) and
     [telescope](https://github.com/nvim-telescope/telescope.nvim) for my
     personal note taking and rely on no proprietary company to enable my
     ability to work in software ... I wanted the same detailed note taking
     independent of any vendor for my music making.  I use DAWs but am not ok
     living in DAWs or any vendor's closed system.

## Installation

```bash
cargo install zim-studio
```

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
├── processed/          # Processed audio (stems, etc)
└── project/            # DAW project files
    ├── ableton/
    └── reaper/
```

## Sidecar Files

`zim update` generates a `.md` sidecar for each audio file with:
- **YAML frontmatter**: Structured metadata (technical specs, tags, etc.)
- **Markdown body**: Free-form notes, ideas, TODO lists

The YAML is designed to be both human-editable and scriptable for automation.
See the [example sidecar](examples/sidecar-example.md) for what this looks like in practice.

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

## License

MIT
