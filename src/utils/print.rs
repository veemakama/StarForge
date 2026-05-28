use colored::*;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use std::time::Duration;

pub fn success(msg: &str) {
    println!("{} {}", "✓".green().bold(), msg);
}

#[allow(dead_code)]
pub fn error(msg: &str) {
    eprintln!("{} {}", "✗".red().bold(), msg);
}

pub fn info(msg: &str) {
    println!("{} {}", "→".cyan(), msg);
}

pub fn warn(msg: &str) {
    println!("{} {}", "⚠".yellow().bold(), msg);
}

pub fn header(msg: &str) {
    println!("\n{}", msg.bright_white().bold().underline());
}

pub fn kv(key: &str, value: &str) {
    println!("  {:<20} {}", key.dimmed(), value.bright_white());
}

pub fn kv_accent(key: &str, value: &str) {
    println!("  {:<20} {}", key.dimmed(), value.cyan().bold());
}

pub fn separator() {
    println!("{}", "─".repeat(60).dimmed());
}

pub fn step(n: usize, total: usize, msg: &str) {
    println!(
        "{} {}",
        format!("[{}/{}]", n, total).dimmed(),
        msg.bright_white()
    );
}

#[allow(dead_code)]
pub fn spinner(msg: &str) -> ProgressBar {
    let pb = ProgressBar::new_spinner();
    pb.enable_steady_tick(Duration::from_millis(120));
    pb.set_style(
        ProgressStyle::with_template("{spinner:.cyan} {msg}")
            .unwrap()
            .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"]),
    );
    pb.set_message(msg.to_string());
    pb
}

pub fn progress_bar(total: u64, msg: &str) -> ProgressBar {
    let pb = ProgressBar::new(total);
    pb.set_style(
        ProgressStyle::with_template(
            "{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta}) {msg}",
        )
        .unwrap()
        .progress_chars("#>-"),
    );
    pb.set_message(msg.to_string());
    pb
}

#[allow(dead_code)]
pub fn multi_progress() -> MultiProgress {
    MultiProgress::new()
}

pub fn verified_badge(verified: bool) -> colored::ColoredString {
    if verified {
        " ✓ verified".green()
    } else {
        "".normal()
    }
}
