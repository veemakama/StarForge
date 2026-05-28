use colored::*;
use std::process::Command;

pub fn info(message: &str) {
    println!("  {} {}", "•".bright_blue(), message);
}

pub fn success(message: &str) {
    println!("  {} {}", "✓".green().bold(), message);
}

pub fn warn(message: &str) {
    eprintln!("  {} {}", "!".yellow().bold(), message);
}

/// Terminal alert with optional OS notification (watchman).
pub fn alert(message: &str) {
    eprintln!(
        "\n  {} {}\n",
        "⚠ ALERT:".red().bold(),
        message.bright_white().bold()
    );
    print!("\x07");
    let _ = std::io::Write::flush(&mut std::io::stdout());
    try_system_notification(message);
}

fn try_system_notification(message: &str) {
    let escaped = message.replace('\\', "\\\\").replace('"', "\\\"");

    #[cfg(target_os = "macos")]
    {
        let script = format!(
            "display notification \"{}\" with title \"StarForge\"",
            escaped
        );
        let _ = Command::new("osascript").args(["-e", &script]).status();
    }

    #[cfg(target_os = "linux")]
    {
        let _ = Command::new("notify-send")
            .args(["StarForge", message])
            .status();
    }
}
