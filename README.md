# ZIM - Terminal-Based Audio Project Scaffold and Metadata System

A Zettelkasten Information system for Music

## Installation

```bash
cargo install zim-studio
```

## Quick Start

```bash
# Initialize ZIM with your music projects directory
zim init ~/MusicProjects

# View configuration
zim config view

# Edit configuration in your editor
zim config edit

# Set configuration values
zim config set default_artist "Your Name"
```

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

TBD