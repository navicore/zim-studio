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
mod wav_metadata;

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
        /// Custom .zimignore template file to use
        #[arg(long)]
        zimignore_template: Option<String>,
        /// Skip creating .zimignore file
        #[arg(long)]
        no_zimignore: bool,
        /// Interactively customize the .zimignore content
        #[arg(short, long)]
        interactive: bool,
    },
    /// Update sidecar metadata files for media assets
    Update {
        /// Path to project (defaults to current directory)
        #[arg(default_value = ".")]
        path: String,
        /// Extra tags to apply to newly created sidecar files
        #[arg(short = 't', long = "tag", action = clap::ArgAction::Append)]
        tags: Vec<String>,
    },
    /// Validate YAML frontmatter in all sidecar files
    Lint {
        /// Path to project (defaults to current directory)
        #[arg(default_value = ".")]
        path: String,
    },
    /// Generate an index.yml file with consolidated track metadata
    Index {
        /// Path to project (defaults to current directory)
        #[arg(default_value = ".")]
        path: String,
    },
    /// Sync technical metadata in sidecar files with current audio file properties
    Sync {
        /// Path to project (defaults to current directory)
        #[arg(default_value = ".")]
        path: String,
    },
    /// Add metadata to sidecar files
    Add {
        #[command(subcommand)]
        action: AddAction,
    },
    /// Tag WAV files with ZIM metadata
    Tag {
        #[command(subcommand)]
        action: TagAction,
    },
    /// Play audio files with integrated player (playlist or mixing mode)
    Play {
        /// Audio file paths (playlist: unlimited files, mixing: up to 3 with --gains)
        files: Vec<String>,
        /// Gain levels for each file (comma-separated, e.g., "0.8,1.2,0.6")
        #[arg(
            short,
            long,
            value_delimiter = ',',
            value_name = "GAIN1,GAIN2,GAIN3",
            help = "Gain levels for each file (0.0-2.0 range)",
            long_help = "Comma-separated gain values for each file (0.0-2.0 range).\nExample: --gains 0.8,1.2,0.6\nDefaults to 1.0 for all files if not specified."
        )]
        gains: Option<Vec<f32>>,
        /// Start interactive mode for browsing and playing
        #[arg(short, long)]
        interactive: bool,
    },
}

#[derive(Subcommand)]
enum AddAction {
    /// Add tags to sidecar files
    Tag {
        /// Path to sidecar file (.md) or directory
        path: String,
        /// Tags to add
        #[arg(short = 't', long = "tag", action = clap::ArgAction::Append, required = true)]
        tags: Vec<String>,
        /// Recursively process subdirectories
        #[arg(short = 'r', long = "recursive")]
        recursive: bool,
    },
}

#[derive(Subcommand)]
enum TagAction {
    /// Create a tagged copy with metadata (original unchanged, new file: *_tagged.wav)
    Add {
        /// WAV file to tag
        file: String,
        /// Project name (auto-detected if not provided)
        #[arg(short, long)]
        project: Option<String>,
    },
    /// Embed metadata directly into existing file (modifies original, backup in /tmp)
    Edit {
        /// WAV file to tag
        file: String,
        /// Project name (auto-detected if not provided)
        #[arg(short, long)]
        project: Option<String>,
        /// Skip backup file creation
        #[arg(long)]
        no_backup: bool,
    },
    /// Display embedded metadata from a WAV file
    Info {
        /// WAV file to read
        file: String,
    },
    /// Copy WAV with lineage tracking (tracks parent/child relationship between files)
    Derive {
        /// Input WAV file
        input: String,
        /// Output WAV file
        output: String,
        /// Type of transformation (e.g., "excerpt", "mix", "master")
        #[arg(short, long, default_value = "process")]
        transform: String,
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
        Commands::New {
            name,
            path,
            zimignore_template,
            no_zimignore,
            interactive,
        } => {
            cli::new::handle_new(
                name.as_deref(),
                path.as_deref(),
                zimignore_template.as_deref(),
                no_zimignore,
                interactive,
            )?;
        }
        Commands::Update { path, tags } => {
            cli::update::handle_update(&path, &tags)?;
        }
        Commands::Lint { path } => {
            cli::lint::handle_lint(&path)?;
        }
        Commands::Index { path } => {
            cli::index::handle_index(&path)?;
        }
        Commands::Sync { path } => {
            cli::sync::handle_sync(&path)?;
        }
        Commands::Add { action } => match action {
            AddAction::Tag {
                path,
                tags,
                recursive,
            } => {
                cli::add::handle_add_tag(&path, &tags, recursive)?;
            }
        },
        Commands::Tag { action } => match action {
            TagAction::Add { file, project } => {
                cli::tag::handle_tag(&file, project)?;
            }
            TagAction::Edit {
                file,
                project,
                no_backup,
            } => {
                cli::tag::handle_tag_edit(&file, project, no_backup)?;
            }
            TagAction::Info { file } => {
                cli::tag::handle_tag_info(&file)?;
            }
            TagAction::Derive {
                input,
                output,
                transform,
            } => {
                cli::tag::handle_tag_derive(&input, &output, &transform)?;
            }
        },
        Commands::Play {
            files,
            gains,
            interactive,
        } => {
            cli::play::handle_play(files, gains, interactive)?;
        }
    }

    Ok(())
}
