use std::error::Error;

// Maximum playlist size to prevent memory issues
const MAX_PLAYLIST_SIZE: usize = 1000;

pub fn handle_play(
    files: Vec<String>,
    gains: Option<Vec<f32>>,
    interactive: bool,
) -> Result<(), Box<dyn Error>> {
    #[cfg(feature = "player")]
    {
        // Validate inputs
        if files.is_empty() && !interactive {
            use owo_colors::OwoColorize;
            println!(
                "{} No files specified. Use {} flag for browser mode.",
                "Error:".red(),
                "--interactive".cyan()
            );
            return Err("No files specified".into());
        }

        // Check file limit for mixing mode (when gains are specified)
        if gains.is_some() && files.len() > 3 {
            use owo_colors::OwoColorize;
            println!(
                "{} Maximum 3 files supported for mixing mode (with --gains).",
                "Error:".red()
            );
            println!(
                "For playlist mode, specify files without --gains (up to {MAX_PLAYLIST_SIZE} files supported)."
            );
            return Err("Too many files for mixing mode".into());
        }

        // Check playlist size limit to prevent memory issues
        if gains.is_none() && files.len() > MAX_PLAYLIST_SIZE {
            use owo_colors::OwoColorize;
            println!(
                "{} Playlist too large: {} files (maximum {} supported).",
                "Error:".red(),
                files.len(),
                MAX_PLAYLIST_SIZE
            );
            return Err("Playlist exceeds maximum size".into());
        }

        // Validate gains if provided
        if let Some(ref g) = gains {
            if !files.is_empty() && g.len() != files.len() {
                use owo_colors::OwoColorize;
                println!(
                    "{} Number of gains ({}) must match number of files ({}).",
                    "Error:".red(),
                    g.len(),
                    files.len()
                );
                return Err("Gain count mismatch".into());
            }

            for (i, gain) in g.iter().enumerate() {
                if *gain < 0.0 || *gain > 2.0 {
                    use owo_colors::OwoColorize;
                    println!(
                        "{} Gain {} ({}) must be between 0.0 and 2.0.",
                        "Error:".red(),
                        i + 1,
                        gain
                    );
                    return Err("Invalid gain value".into());
                }
            }
        }

        crate::player::run(files, gains, interactive)
    }

    #[cfg(not(feature = "player"))]
    {
        let _ = files;
        let _ = gains;
        let _ = interactive;
        use owo_colors::OwoColorize;
        println!("{} {}", "🎵".cyan(), "Audio Player".bold());
        println!();
        println!(
            "{} The audio player requires the 'player' feature to be enabled.",
            "Note:".yellow()
        );
        println!();
        println!("To enable it, install with:");
        println!("  {}", "cargo install zim-studio --features player".cyan());
        println!();
        println!("Or if building from source:");
        println!("  {}", "cargo build --release --features player".cyan());

        Ok(())
    }
}
