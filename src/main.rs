use clap::{CommandFactory, Parser, Subcommand};
use clap_complete::{Generator, Shell, generate};
use std::error::Error;
use std::io;

mod cli;
mod config;

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
    /// Initialize ZIM with a root directory for all music projects
    Init {
        /// Root directory for all music projects
        root_dir: String,
    },
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
}

#[derive(Subcommand)]
enum ConfigAction {
    /// View current configuration
    View,
    /// Set a configuration value
    Set {
        /// Configuration key
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
        Commands::Init { root_dir } => {
            cli::init::handle_init(&root_dir)?;
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
    }

    Ok(())
}
