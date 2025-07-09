//! ZIM Studio - Terminal-based audio project management and player.
//!
//! This application provides two main functionalities:
//!
//! 1. **Project Management**: A scaffolding system for organizing audio projects
//!    with consistent folder structures, metadata management via markdown sidecar
//!    files, and YAML frontmatter validation.
//!
//! 2. **Audio Player** (optional feature): A terminal-based audio player with
//!    waveform visualization, designed for fast sample browsing and editing
//!    workflows. The player supports mark/loop functionality and can export
//!    selections as new files.
//!
//! The tool is designed for musicians and audio engineers who prefer working
//! in the terminal and want a fast, keyboard-driven workflow for managing and
//! auditioning audio files.

use clap::{CommandFactory, Parser, Subcommand, builder::PossibleValuesParser};
use clap_complete::{Generator, Shell, generate};
use std::error::Error;
use std::io;

mod cli;
mod config;
mod media;
mod project;
mod templates;

#[cfg(feature = "player")]
mod player;

#[derive(Parser)]
#[command(name = "zim")]
#[command(about = "Terminal-based audio project scaffold and metadata system")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize ZIM configuration
    Init,
    /// Show current configuration
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
    /// Generate shell completions
    Completions {
        /// The shell to generate completions for
        #[arg(value_enum)]
        shell: Shell,
    },
    /// Create a new audio project scaffold
    New {
        /// Project name (optional, auto-generates if not provided)
        name: Option<String>,
        /// Parent directory for the project (defaults to current directory)
        #[arg(short, long)]
        path: Option<String>,
    },
    /// Update sidecar metadata files for media assets
    Update {
        /// Path to project (defaults to current directory)
        #[arg(default_value = ".")]
        path: String,
    },
    /// Validate YAML frontmatter in all sidecar files
    Lint {
        /// Path to project (defaults to current directory)
        #[arg(default_value = ".")]
        path: String,
    },
    /// Play audio files with integrated player (coming soon)
    Play {
        /// Search pattern or file path
        pattern: Option<String>,
        /// Start interactive mode for browsing and playing
        #[arg(short, long)]
        interactive: bool,
    },
}

#[derive(Subcommand)]
enum ConfigAction {
    /// View current configuration
    View,
    /// Set a configuration value
    Set {
        /// Configuration key
        #[arg(value_parser = PossibleValuesParser::new(["root_dir", "default_artist", "normalize_project_names"]))]
        key: String,
        /// Configuration value
        value: String,
    },
    /// Edit configuration file in your editor
    Edit,
}

fn print_completions<G: Generator>(generator: G, cmd: &mut clap::Command) {
    generate(
        generator,
        cmd,
        cmd.get_name().to_string(),
        &mut io::stdout(),
    );
}

fn main() -> Result<(), Box<dyn Error>> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Init => {
            cli::init::handle_init()?;
        }
        Commands::Config { action } => match action {
            ConfigAction::View => {
                cli::config::handle_config_view()?;
            }
            ConfigAction::Set { key, value } => {
                cli::config::handle_config_set(&key, &value)?;
            }
            ConfigAction::Edit => {
                cli::config::handle_config_edit()?;
            }
        },
        Commands::Completions { shell } => {
            let mut cmd = Cli::command();
            print_completions(shell, &mut cmd);
        }
        Commands::New { name, path } => {
            cli::new::handle_new(name.as_deref(), path.as_deref())?;
        }
        Commands::Update { path } => {
            cli::update::handle_update(&path)?;
        }
        Commands::Lint { path } => {
            cli::lint::handle_lint(&path)?;
        }
        Commands::Play {
            pattern,
            interactive,
        } => {
            cli::play::handle_play(pattern.as_deref(), interactive)?;
        }
    }

    Ok(())
}
