use anyhow::Result;
use colored::*;
use std::collections::VecDeque;
use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;

pub struct Repl<R>
where
    R: ReplRunner,
{
    runner: R,
    options: ReplOptions,
}

#[derive(Debug, Clone)]
pub struct ReplOptions {
    pub history_enabled: bool,
    pub history_path: PathBuf,
    pub max_history_lines: usize,
}

impl Default for ReplOptions {
    fn default() -> Self {
        let mut path = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        path.push(".starforge");
        path.push("repl_history");
        Self {
            history_enabled: true,
            history_path: path,
            max_history_lines: 1000,
        }
    }
}

pub trait ReplRunner {
    fn run_invocation(&mut self, function: &str, args: &[String]) -> Result<String>;
}

impl<R> Repl<R>
where
    R: ReplRunner,
{
    pub fn new(runner: R) -> Self {
        Self {
            runner,
            options: ReplOptions::default(),
        }
    }

    pub fn with_options(runner: R, options: ReplOptions) -> Self {
        Self { runner, options }
    }

    pub fn run(mut self) -> Result<()> {
        println!(
            "  {} {}",
            "StarForge Shell".bright_cyan().bold(),
            "(type :help for commands)".dimmed()
        );

        let stdin = io::stdin();
        let mut buffer = String::new();
        let mut history = self.load_history()?;

        loop {
            buffer.clear();
            print!("{}", "> ".bright_green().bold());
            io::stdout().flush()?;

            if stdin.read_line(&mut buffer)? == 0 {
                break;
            }

            let line = buffer.trim();
            if line.is_empty() {
                continue;
            }

            if line == ":q" || line == ":quit" || line == ":exit" {
                break;
            }

            if line == ":help" {
                println!("  {}", "Commands:".bold());
                println!("    :help              Show help");
                println!("    :quit | :exit      Exit shell");
                println!("    fn(arg1, arg2)     Invoke a contract function");
                continue;
            }

            self.push_history(&mut history, line.to_string());
            let (function, args) = parse_invocation(line)?;
            match self.runner.run_invocation(&function, &args) {
                Ok(out) => println!("{}", out),
                Err(e) => eprintln!("  {} {}", "✗".red().bold(), e),
            }
        }

        self.save_history(&history)?;
        Ok(())
    }

    fn load_history(&self) -> Result<VecDeque<String>> {
        if !self.options.history_enabled {
            return Ok(VecDeque::new());
        }

        let content = match fs::read_to_string(&self.options.history_path) {
            Ok(content) => content,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(VecDeque::new()),
            Err(e) => return Err(e.into()),
        };

        let mut lines: VecDeque<String> = content.lines().map(|l| l.to_string()).collect();
        trim_history(&mut lines, self.options.max_history_lines);
        Ok(lines)
    }

    fn save_history(&self, history: &VecDeque<String>) -> Result<()> {
        if !self.options.history_enabled {
            return Ok(());
        }

        if let Some(parent) = self.options.history_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let mut out = String::new();
        for line in history {
            out.push_str(line);
            out.push('\n');
        }
        fs::write(&self.options.history_path, out)?;
        Ok(())
    }

    fn push_history(&self, history: &mut VecDeque<String>, line: String) {
        if !self.options.history_enabled {
            return;
        }
        history.push_back(line);
        trim_history(history, self.options.max_history_lines);
    }
}

fn trim_history(history: &mut VecDeque<String>, max_lines: usize) {
    if max_lines == 0 {
        history.clear();
        return;
    }
    while history.len() > max_lines {
        history.pop_front();
    }
}

fn parse_invocation(input: &str) -> Result<(String, Vec<String>)> {
    let open = input
        .find('(')
        .ok_or_else(|| anyhow::anyhow!("Expected invocation like fn(\"arg\")"))?;
    let close = input
        .rfind(')')
        .ok_or_else(|| anyhow::anyhow!("Missing closing ')'"))?;
    if close < open {
        anyhow::bail!("Invalid invocation");
    }

    let function = input[..open].trim();
    if function.is_empty() {
        anyhow::bail!("Missing function name");
    }

    let args_raw = input[open + 1..close].trim();
    let args = split_args(args_raw)?;
    Ok((function.to_string(), args))
}

fn split_args(input: &str) -> Result<Vec<String>> {
    if input.is_empty() {
        return Ok(Vec::new());
    }

    let mut args = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut quote_char = '\0';
    let mut escape = false;

    for ch in input.chars() {
        if escape {
            current.push(ch);
            escape = false;
            continue;
        }

        if ch == '\\' {
            escape = true;
            continue;
        }

        if in_quotes {
            if ch == quote_char {
                in_quotes = false;
                continue;
            }
            current.push(ch);
            continue;
        }

        if ch == '"' || ch == '\'' {
            in_quotes = true;
            quote_char = ch;
            continue;
        }

        if ch == ',' {
            args.push(current.trim().to_string());
            current.clear();
            continue;
        }

        current.push(ch);
    }

    if in_quotes {
        anyhow::bail!("Unclosed quote in arguments");
    }

    if escape {
        anyhow::bail!("Trailing escape in arguments");
    }

    args.push(current.trim().to_string());
    Ok(args)
}
