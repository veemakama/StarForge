use anyhow::Result;
use tracing::Level;
use tracing_appender::rolling;
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, Layer};

/// Output format for log messages.
#[derive(Debug, Clone, PartialEq)]
pub enum LogFormat {
    /// Human-readable coloured output (default for terminals)
    Human,
    /// Newline-delimited JSON (useful for CI/CD and log aggregators)
    Json,
}

/// Configuration for the logging subsystem.
pub struct LogConfig {
    /// Minimum log level to emit (default: `warn` for normal use, `debug` with `RUST_LOG`)
    pub level: Level,
    /// Output format
    pub format: LogFormat,
    /// Optional directory to write rolling log files into
    pub log_dir: Option<std::path::PathBuf>,
    /// Log file prefix (e.g. "starforge")
    pub file_prefix: String,
}

impl Default for LogConfig {
    fn default() -> Self {
        Self {
            level: Level::WARN,
            format: LogFormat::Human,
            log_dir: None,
            file_prefix: "starforge".to_string(),
        }
    }
}

/// Initialise the global tracing subscriber.
///
/// Call this once at the start of `main()` before any commands run.
/// The `RUST_LOG` environment variable overrides `config.level` when set.
///
/// # Examples
/// ```no_run
/// use starforge::utils::logging::{LogConfig, LogFormat, init};
/// init(LogConfig { format: LogFormat::Json, ..Default::default() }).unwrap();
/// ```
pub fn init(config: LogConfig) -> Result<()> {
    // RUST_LOG takes precedence; fall back to the configured level.
    let env_filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(config.level.as_str()));

    match (config.format, config.log_dir) {
        // ── JSON + file rotation ──────────────────────────────────────────
        (LogFormat::Json, Some(dir)) => {
            let file_appender = rolling::daily(&dir, &config.file_prefix);
            let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

            let file_layer = fmt::layer()
                .json()
                .with_writer(non_blocking)
                .with_filter(env_filter.clone());

            let stderr_layer = fmt::layer()
                .json()
                .with_writer(std::io::stderr)
                .with_filter(env_filter);

            tracing_subscriber::registry()
                .with(file_layer)
                .with(stderr_layer)
                .try_init()
                .map_err(|e| anyhow::anyhow!("Failed to init logger: {}", e))?;
        }

        // ── JSON, stderr only ─────────────────────────────────────────────
        (LogFormat::Json, None) => {
            let layer = fmt::layer()
                .json()
                .with_writer(std::io::stderr)
                .with_filter(env_filter);

            tracing_subscriber::registry()
                .with(layer)
                .try_init()
                .map_err(|e| anyhow::anyhow!("Failed to init logger: {}", e))?;
        }

        // ── Human + file rotation ─────────────────────────────────────────
        (LogFormat::Human, Some(dir)) => {
            let file_appender = rolling::daily(&dir, &config.file_prefix);
            let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

            let file_layer = fmt::layer()
                .with_ansi(false)
                .with_writer(non_blocking)
                .with_filter(env_filter.clone());

            let stderr_layer = fmt::layer()
                .with_writer(std::io::stderr)
                .with_filter(env_filter);

            tracing_subscriber::registry()
                .with(file_layer)
                .with(stderr_layer)
                .try_init()
                .map_err(|e| anyhow::anyhow!("Failed to init logger: {}", e))?;
        }

        // ── Human, stderr only (default) ──────────────────────────────────
        (LogFormat::Human, None) => {
            let layer = fmt::layer()
                .with_writer(std::io::stderr)
                .with_filter(env_filter);

            tracing_subscriber::registry()
                .with(layer)
                .try_init()
                .map_err(|e| anyhow::anyhow!("Failed to init logger: {}", e))?;
        }
    }

    Ok(())
}

/// Build a `LogConfig` from CLI flags / environment.
///
/// - `--log-format json` → `LogFormat::Json`
/// - `--log-dir <path>`  → file rotation into that directory
/// - `RUST_LOG`          → overrides level at the filter level
pub fn config_from_env(format: Option<&str>, log_dir: Option<std::path::PathBuf>) -> LogConfig {
    let format = match format {
        Some("json") => LogFormat::Json,
        _ => LogFormat::Human,
    };

    LogConfig {
        format,
        log_dir,
        ..Default::default()
    }
}
