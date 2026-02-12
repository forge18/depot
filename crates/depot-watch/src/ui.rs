use colored::*;
use std::time::{SystemTime, UNIX_EPOCH};

/// Terminal UI utilities for better output formatting
pub struct UI;

impl UI {
    /// Print a formatted status message
    pub fn status(message: &str) {
        let timestamp = Self::timestamp();
        println!(
            "{} {} {}",
            timestamp.bright_black(),
            "●".bright_cyan(),
            message.bright_white()
        );
    }

    /// Print an error message
    pub fn error(message: &str) {
        let timestamp = Self::timestamp();
        eprintln!(
            "{} {} {}",
            timestamp.bright_black(),
            "✗".red(),
            message.red()
        );
    }

    /// Print a warning message
    pub fn warning(message: &str) {
        let timestamp = Self::timestamp();
        println!(
            "{} {} {}",
            timestamp.bright_black(),
            "⚠".yellow(),
            message.yellow()
        );
    }

    /// Print an info message
    pub fn info(message: &str) {
        let timestamp = Self::timestamp();
        println!(
            "{} {} {}",
            timestamp.bright_black(),
            "ℹ".blue(),
            message.blue()
        );
    }

    /// Print a file change notification
    pub fn file_changed(path: &str) {
        let timestamp = Self::timestamp();
        println!(
            "{} {} {} {}",
            timestamp.bright_black(),
            "📝".bright_yellow(),
            "File changed:".bright_white(),
            path.bright_cyan()
        );
    }

    /// Print a restart notification
    pub fn restarting() {
        let timestamp = Self::timestamp();
        println!(
            "{} {} {}",
            timestamp.bright_black(),
            "🔄".bright_magenta(),
            "Restarting...".bright_white()
        );
    }

    /// Print server start message
    pub fn server_start(watching: &str, command: &str) {
        println!(
            "\n{}",
            "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".bright_black()
        );
        println!(
            "{} {}",
            "🔍".bright_cyan(),
            "Starting dev server".bright_white().bold()
        );
        println!(
            "{} {}",
            "   Watching:".bright_black(),
            watching.bright_white()
        );
        println!(
            "{} {}",
            "   Command:".bright_black(),
            command.bright_white()
        );
        println!(
            "{} {}",
            "   Press".bright_black(),
            "Ctrl+C".bright_red().bold()
        );
        println!(
            "{}",
            "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".bright_black()
        );
        println!();
    }

    /// Print server stop message
    pub fn server_stop() {
        println!(
            "\n{}",
            "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".bright_black()
        );
        println!(
            "{} {}",
            "🛑".bright_red(),
            "Stopping dev server".bright_white().bold()
        );
        println!(
            "{}",
            "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".bright_black()
        );
    }

    /// Clear the screen
    pub fn clear() {
        if std::env::var("TERM").is_ok() {
            print!("\x1B[2J\x1B[1;1H");
        }
    }

    /// Get formatted timestamp
    fn timestamp() -> String {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let time = now % 86400; // Seconds since midnight
        let hours = time / 3600;
        let minutes = (time % 3600) / 60;
        let seconds = time % 60;
        format!("{:02}:{:02}:{:02}", hours, minutes, seconds)
    }
}
