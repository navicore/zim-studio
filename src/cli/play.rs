use std::error::Error;

pub fn handle_play(pattern: Option<&str>, interactive: bool) -> Result<(), Box<dyn Error>> {
    #[cfg(feature = "player")]
    {
        crate::player::run(pattern, interactive)
    }

    #[cfg(not(feature = "player"))]
    {
        let _ = pattern;
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
