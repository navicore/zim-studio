use std::error::Error;

pub fn handle_play(
    files: Vec<String>,
    gains: Vec<Option<f32>>,
    pans: Vec<Option<f32>>,
    interactive: bool,
) -> Result<(), Box<dyn Error>> {
    #[cfg(feature = "player")]
    {
        // Convert Vec<Option<f32>> to Option<Vec<f32>>, taking only the values we have files for
        let gains: Option<Vec<f32>> = if gains.iter().take(files.len()).any(|g| g.is_some()) {
            Some(
                gains
                    .into_iter()
                    .take(files.len())
                    .map(|g| g.unwrap_or(1.0)) // Default to 1.0 if not specified
                    .collect(),
            )
        } else {
            None
        };

        let pans: Option<Vec<f32>> = if pans.iter().take(files.len()).any(|p| p.is_some()) {
            Some(
                pans.into_iter()
                    .take(files.len())
                    .map(|p| {
                        let pan_0_to_1 = p.unwrap_or(0.5); // Default to 0.5 (center) if not specified
                        // Convert 0.0-1.0 range to -1.0-1.0 range for audio engine
                        (pan_0_to_1 - 0.5) * 2.0
                    })
                    .collect(),
            )
        } else {
            None
        };

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

        if files.len() > 3 {
            use owo_colors::OwoColorize;
            println!("{} Maximum 3 files supported for mixing.", "Error:".red());
            return Err("Too many files specified".into());
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

        // Validate pans if provided
        if let Some(ref p) = pans {
            if !files.is_empty() && p.len() != files.len() {
                use owo_colors::OwoColorize;
                println!(
                    "{} Number of pans ({}) must match number of files ({}).",
                    "Error:".red(),
                    p.len(),
                    files.len()
                );
                return Err("Pan count mismatch".into());
            }

            for (i, pan) in p.iter().enumerate() {
                if *pan < -1.0 || *pan > 1.0 {
                    use owo_colors::OwoColorize;
                    println!(
                        "{} Pan {} ({}) must be between -1.0 and 1.0.",
                        "Error:".red(),
                        i + 1,
                        pan
                    );
                    return Err("Invalid pan value".into());
                }
            }
        }

        crate::player::run(files, gains, pans, interactive)
    }

    #[cfg(not(feature = "player"))]
    {
        let _ = files;
        let _ = gains;
        let _ = pans;
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
