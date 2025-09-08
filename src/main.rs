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

use flag_rs::{ArgValidator, CommandBuilder, CompletionResult, Flag, FlagType, FlagValue, Shell};
use std::error::Error;

mod cli;
mod config;
mod media;
mod project;
mod templates;

#[cfg(feature = "player")]
mod player;

fn build_root_command() -> flag_rs::Command {
    CommandBuilder::new("zim")
        .short("Terminal-based audio project scaffold and metadata system")
        .subcommand(
            CommandBuilder::new("init")
                .short("Initialize ZIM configuration")
                .run(|_ctx| {
                    cli::init::handle_init().map_err(|e| flag_rs::Error::Validation(e.to_string()))?;
                    Ok(())
                })
                .build()
        )
        .subcommand(
            CommandBuilder::new("config")
                .short("Show current configuration")
                .subcommand(
                    CommandBuilder::new("view")
                        .short("View current configuration")
                        .run(|_ctx| {
                            cli::config::handle_config_view().map_err(|e| flag_rs::Error::Validation(e.to_string()))?;
                            Ok(())
                        })
                        .build()
                )
                .subcommand(
                    CommandBuilder::new("set")
                        .short("Set a configuration value")
                        .args(ArgValidator::ExactArgs(2))
                        .run(|ctx| {
                            let args = ctx.args();
                            if args.len() != 2 {
                                return Err(flag_rs::Error::ArgumentValidation {
                                    message: "Expected key and value arguments".to_string(),
                                    expected: "2 arguments".to_string(),
                                    received: args.len()
                                });
                            }
                            let key = &args[0];
                            let value = &args[1];
                            cli::config::handle_config_set(key, value).map_err(|e| flag_rs::Error::Validation(e.to_string()))?;
                            Ok(())
                        })
                        .build()
                )
                .subcommand(
                    CommandBuilder::new("edit")
                        .short("Edit configuration file in your editor")
                        .run(|_ctx| {
                            cli::config::handle_config_edit().map_err(|e| flag_rs::Error::Validation(e.to_string()))?;
                            Ok(())
                        })
                        .build()
                )
                .build()
        )
        .subcommand(
            CommandBuilder::new("completions")
                .short("Generate shell completions")
                .args(ArgValidator::ExactArgs(1))
                .arg_completion(|_ctx, prefix| {
                    let shells = vec![
                        ("bash", "Bash shell completion"),
                        ("zsh", "Zsh shell completion"),
                        ("fish", "Fish shell completion"),
                    ];

                    let mut result = CompletionResult::new();
                    for (shell, description) in shells {
                        if shell.starts_with(prefix) {
                            result = result.add_with_description(shell.to_string(), description.to_string());
                        }
                    }

                    Ok(result)
                })
                .run(|ctx| {
                    let args = ctx.args();
                    if args.is_empty() {
                        return Err(flag_rs::Error::ArgumentValidation {
                            message: "Shell argument required".to_string(),
                            expected: "1 argument".to_string(),
                            received: 0
                        });
                    }
                    let shell_name = &args[0];
                    let shell = match shell_name.as_str() {
                        "bash" => Shell::Bash,
                        "zsh" => Shell::Zsh,
                        "fish" => Shell::Fish,
                        _ => {
                            return Err(flag_rs::Error::ArgumentValidation {
                                message: format!("Unsupported shell: {shell_name}"),
                                expected: "bash, zsh, or fish".to_string(),
                                received: 1
                            });
                        }
                    };

                    let root = build_root_command();
                    let script = root.generate_completion(shell);
                    println!("{script}");
                    Ok(())
                })
                .build()
        )
        .subcommand(
            CommandBuilder::new("new")
                .short("Create a new audio project scaffold")
                .args(ArgValidator::RangeArgs(0, 1))
                .flag(
                    Flag::new("path")
                        .short('p')
                        .usage("Parent directory for the project (defaults to current directory)")
                        .value_type(FlagType::String)
                )
                .flag(
                    Flag::new("zimignore-template")
                        .usage("Custom .zimignore template file to use")
                        .value_type(FlagType::String)
                )
                .flag(
                    Flag::new("no-zimignore")
                        .usage("Skip creating .zimignore file")
                        .value_type(FlagType::Bool)
                        .default(FlagValue::Bool(false))
                )
                .flag(
                    Flag::new("interactive")
                        .short('i')
                        .usage("Interactively customize the .zimignore content")
                        .value_type(FlagType::Bool)
                        .default(FlagValue::Bool(false))
                )
                .run(|ctx| {
                    let name = if ctx.args().is_empty() { None } else { Some(ctx.args()[0].as_str()) };
                    let path = ctx.flag("path");
                    let zimignore_template = ctx.flag("zimignore-template");
                    let no_zimignore = ctx.flag_bool("no-zimignore").unwrap_or(false);
                    let interactive = ctx.flag_bool("interactive").unwrap_or(false);

                    cli::new::handle_new(
                        name,
                        path.map(|x| x.as_str()),
                        zimignore_template.map(|x| x.as_str()),
                        no_zimignore,
                        interactive,
                    ).map_err(|e| flag_rs::Error::Validation(e.to_string()))?;
                    Ok(())
                })
                .build()
        )
        .subcommand(
            CommandBuilder::new("update")
                .short("Update sidecar metadata files for media assets")
                .args(ArgValidator::RangeArgs(0, 1))
                .run(|ctx| {
                    let path = if ctx.args().is_empty() { "." } else { &ctx.args()[0] };
                    cli::update::handle_update(path).map_err(|e| flag_rs::Error::Validation(e.to_string()))?;
                    Ok(())
                })
                .build()
        )
        .subcommand(
            CommandBuilder::new("lint")
                .short("Validate YAML frontmatter in all sidecar files")
                .args(ArgValidator::RangeArgs(0, 1))
                .run(|ctx| {
                    let path = if ctx.args().is_empty() { "." } else { &ctx.args()[0] };
                    cli::lint::handle_lint(path).map_err(|e| flag_rs::Error::Validation(e.to_string()))?;
                    Ok(())
                })
                .build()
        )
        .subcommand(
            CommandBuilder::new("play")
                .short("Play audio files with integrated player (supports mixing up to 3 files)")
                .flag(
                    Flag::new("file1")
                        .usage("First audio file")
                        .value_type(FlagType::String)
                )
                .flag(
                    Flag::new("file2")
                        .usage("Second audio file (for mixing)")
                        .value_type(FlagType::String)
                )
                .flag(
                    Flag::new("file3")
                        .usage("Third audio file (for mixing)")
                        .value_type(FlagType::String)
                )
                .flag(
                    Flag::new("file1-gain")
                        .usage("Gain level for first file (0.0-2.0 range). Common: 1.0=unity, 0.8=quieter, 1.2=louder")
                        .value_type(FlagType::String)
                )
                .flag_completion("file1-gain", |_ctx, _prefix| {
                    let mut result = CompletionResult::new();
                    result = result.add_with_description("0.5", "Quiet (-6dB)");
                    result = result.add_with_description("0.8", "Slightly quieter");
                    result = result.add_with_description("1.0", "Unity gain (default)");
                    result = result.add_with_description("1.2", "Slightly louder");
                    result = result.add_with_description("1.5", "Moderately loud (+3.5dB)");
                    result = result.add_with_description("2.0", "Maximum (+6dB)");
                    Ok(result)
                })
                .flag(
                    Flag::new("file2-gain")
                        .usage("Gain level for second file (0.0-2.0 range). Common: 1.0=unity, 0.8=quieter, 1.2=louder")
                        .value_type(FlagType::String)
                )
                .flag_completion("file2-gain", |_ctx, _prefix| {
                    let mut result = CompletionResult::new();
                    result = result.add_with_description("0.5", "Quiet (-6dB)");
                    result = result.add_with_description("0.8", "Slightly quieter");
                    result = result.add_with_description("1.0", "Unity gain (default)");
                    result = result.add_with_description("1.2", "Slightly louder");
                    result = result.add_with_description("1.5", "Moderately loud (+3.5dB)");
                    result = result.add_with_description("2.0", "Maximum (+6dB)");
                    Ok(result)
                })
                .flag(
                    Flag::new("file3-gain")
                        .usage("Gain level for third file (0.0-2.0 range). Common: 1.0=unity, 0.8=quieter, 1.2=louder")
                        .value_type(FlagType::String)
                )
                .flag_completion("file3-gain", |_ctx, _prefix| {
                    let mut result = CompletionResult::new();
                    result = result.add_with_description("0.5", "Quiet (-6dB)");
                    result = result.add_with_description("0.8", "Slightly quieter");
                    result = result.add_with_description("1.0", "Unity gain (default)");
                    result = result.add_with_description("1.2", "Slightly louder");
                    result = result.add_with_description("1.5", "Moderately loud (+3.5dB)");
                    result = result.add_with_description("2.0", "Maximum (+6dB)");
                    Ok(result)
                })
                .flag(
                    Flag::new("file1-pan")
                        .usage("Pan position for first file (0.0-1.0 range). 0.0=left, 0.5=center, 1.0=right")
                        .value_type(FlagType::String)
                )
                .flag_completion("file1-pan", |_ctx, _prefix| {
                    let mut result = CompletionResult::new();
                    result = result.add_with_description("0.0", "Hard left");
                    result = result.add_with_description("0.2", "Left bias");
                    result = result.add_with_description("0.5", "Center (default)");
                    result = result.add_with_description("0.8", "Right bias");
                    result = result.add_with_description("1.0", "Hard right");
                    Ok(result)
                })
                .flag(
                    Flag::new("file2-pan")
                        .usage("Pan position for second file (0.0-1.0 range). 0.0=left, 0.5=center, 1.0=right")
                        .value_type(FlagType::String)
                )
                .flag_completion("file2-pan", |_ctx, _prefix| {
                    let mut result = CompletionResult::new();
                    result = result.add_with_description("0.0", "Hard left");
                    result = result.add_with_description("0.2", "Left bias");
                    result = result.add_with_description("0.5", "Center (default)");
                    result = result.add_with_description("0.8", "Right bias");
                    result = result.add_with_description("1.0", "Hard right");
                    Ok(result)
                })
                .flag(
                    Flag::new("file3-pan")
                        .usage("Pan position for third file (0.0-1.0 range). 0.0=left, 0.5=center, 1.0=right")
                        .value_type(FlagType::String)
                )
                .flag_completion("file3-pan", |_ctx, _prefix| {
                    let mut result = CompletionResult::new();
                    result = result.add_with_description("0.0", "Hard left");
                    result = result.add_with_description("0.2", "Left bias");
                    result = result.add_with_description("0.5", "Center (default)");
                    result = result.add_with_description("0.8", "Right bias");
                    result = result.add_with_description("1.0", "Hard right");
                    Ok(result)
                })
                .flag(
                    Flag::new("interactive")
                        .short('i')
                        .usage("Start interactive mode for browsing and playing")
                        .value_type(FlagType::Bool)
                        .default(FlagValue::Bool(false))
                )
                .flag_completion("file1-gain", |_ctx, _prefix| {
                    let mut result = CompletionResult::new();
                    result = result.add_with_description("0.5", "Quiet (-6dB)");
                    result = result.add_with_description("0.8", "Slightly quieter");
                    result = result.add_with_description("1.0", "Unity gain (default)");
                    result = result.add_with_description("1.2", "Slightly louder");
                    result = result.add_with_description("1.5", "Moderately loud (+3.5dB)");
                    result = result.add_with_description("2.0", "Maximum (+6dB)");
                    Ok(result)
                })
                .flag_completion("file2-gain", |_ctx, _prefix| {
                    let mut result = CompletionResult::new();
                    result = result.add_with_description("0.5", "Quiet (-6dB)");
                    result = result.add_with_description("0.8", "Slightly quieter");
                    result = result.add_with_description("1.0", "Unity gain (default)");
                    result = result.add_with_description("1.2", "Slightly louder");
                    result = result.add_with_description("1.5", "Moderately loud (+3.5dB)");
                    result = result.add_with_description("2.0", "Maximum (+6dB)");
                    Ok(result)
                })
                .flag_completion("file3-gain", |_ctx, _prefix| {
                    let mut result = CompletionResult::new();
                    result = result.add_with_description("0.5", "Quiet (-6dB)");
                    result = result.add_with_description("0.8", "Slightly quieter");
                    result = result.add_with_description("1.0", "Unity gain (default)");
                    result = result.add_with_description("1.2", "Slightly louder");
                    result = result.add_with_description("1.5", "Moderately loud (+3.5dB)");
                    result = result.add_with_description("2.0", "Maximum (+6dB)");
                    Ok(result)
                })
                .flag_completion("file1-pan", |_ctx, _prefix| {
                    let mut result = CompletionResult::new();
                    result = result.add_with_description("0.0", "Hard left");
                    result = result.add_with_description("0.2", "Left bias");
                    result = result.add_with_description("0.5", "Center (default)");
                    result = result.add_with_description("0.8", "Right bias");
                    result = result.add_with_description("1.0", "Hard right");
                    Ok(result)
                })
                .flag_completion("file2-pan", |_ctx, _prefix| {
                    let mut result = CompletionResult::new();
                    result = result.add_with_description("0.0", "Hard left");
                    result = result.add_with_description("0.2", "Left bias");
                    result = result.add_with_description("0.5", "Center (default)");
                    result = result.add_with_description("0.8", "Right bias");
                    result = result.add_with_description("1.0", "Hard right");
                    Ok(result)
                })
                .flag_completion("file3-pan", |_ctx, _prefix| {
                    let mut result = CompletionResult::new();
                    result = result.add_with_description("0.0", "Hard left");
                    result = result.add_with_description("0.2", "Left bias");
                    result = result.add_with_description("0.5", "Center (default)");
                    result = result.add_with_description("0.8", "Right bias");
                    result = result.add_with_description("1.0", "Hard right");
                    Ok(result)
                })
                .run(|ctx| {
                    // Combine named arguments
                    let mut all_files = Vec::new();
                    if let Some(f) = ctx.flag("file1") {
                        all_files.push(f.to_string());
                    }
                    if let Some(f) = ctx.flag("file2") {
                        all_files.push(f.to_string());
                    }
                    if let Some(f) = ctx.flag("file3") {
                        all_files.push(f.to_string());
                    }

                    // Parse and collect gains
                    let mut gains = Vec::new();
                    if let Some(g) = ctx.flag("file1-gain") {
                        let parsed = g.parse::<f32>().map_err(|_| flag_rs::Error::ArgumentParsing("Invalid gain value".to_string()))?;
                        if !(0.0..=2.0).contains(&parsed) {
                            return Err(flag_rs::Error::Validation("Gain must be between 0.0 and 2.0".to_string()));
                        }
                        gains.push(Some(parsed));
                    } else {
                        gains.push(None);
                    }
                    if let Some(g) = ctx.flag("file2-gain") {
                        let parsed = g.parse::<f32>().map_err(|_| flag_rs::Error::ArgumentParsing("Invalid gain value".to_string()))?;
                        if !(0.0..=2.0).contains(&parsed) {
                            return Err(flag_rs::Error::Validation("Gain must be between 0.0 and 2.0".to_string()));
                        }
                        gains.push(Some(parsed));
                    } else {
                        gains.push(None);
                    }
                    if let Some(g) = ctx.flag("file3-gain") {
                        let parsed = g.parse::<f32>().map_err(|_| flag_rs::Error::ArgumentParsing("Invalid gain value".to_string()))?;
                        if !(0.0..=2.0).contains(&parsed) {
                            return Err(flag_rs::Error::Validation("Gain must be between 0.0 and 2.0".to_string()));
                        }
                        gains.push(Some(parsed));
                    } else {
                        gains.push(None);
                    }

                    // Parse and collect pans
                    let mut pans = Vec::new();
                    if let Some(p) = ctx.flag("file1-pan") {
                        let parsed = p.parse::<f32>().map_err(|_| flag_rs::Error::ArgumentParsing("Invalid pan value".to_string()))?;

                        if !(0.0..=1.0).contains(&parsed) {
                            return Err(flag_rs::Error::Validation("Pan must be between 0.0 and 1.0".to_string()));
                        }
                        pans.push(Some(parsed));
                    } else {
                        pans.push(None);
                    }
                    if let Some(p) = ctx.flag("file2-pan") {
                        let parsed = p.parse::<f32>().map_err(|_| flag_rs::Error::ArgumentParsing("Invalid pan value".to_string()))?;
                        if !(0.0..=1.0).contains(&parsed) {
                            return Err(flag_rs::Error::Validation("Pan must be between 0.0 and 1.0".to_string()));
                        }
                        pans.push(Some(parsed));
                    } else {
                        pans.push(None);
                    }
                    if let Some(p) = ctx.flag("file3-pan") {
                        let parsed = p.parse::<f32>().map_err(|_| flag_rs::Error::ArgumentParsing("Invalid pan value".to_string()))?;

                        if !(0.0..=1.0).contains(&parsed) {
                            return Err(flag_rs::Error::Validation("Pan must be between 0.0 and 1.0".to_string()));
                        }
                        pans.push(Some(parsed));
                    } else {
                        pans.push(None);
                    }

                    let interactive = ctx.flag_bool("interactive").unwrap_or(false);

                    cli::play::handle_play(all_files, gains, pans, interactive).map_err(|e| flag_rs::Error::Validation(e.to_string()))?;
                    Ok(())
                })
                .build()
        )
        .build()
}

fn main() -> Result<(), Box<dyn Error>> {
    let app = build_root_command();

    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Err(e) = app.execute(args) {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
    Ok(())
}
