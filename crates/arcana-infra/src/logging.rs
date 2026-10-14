//! Logging setup (the Serilog configuration in C# `AddArcanaServices`): console plus a
//! daily-rolling file `<dir>/app-<date>.log` keeping 7 files.

use std::path::{Path, PathBuf};

use arcana_core::{AppError, AppResult, ErrorCode};
use tracing_appender::non_blocking::WorkerGuard;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::EnvFilter;
use tracing_subscriber::fmt::writer::MakeWriterExt;

/// Files kept by the rolling appender (C# `retainedFileCountLimit: 7`).
pub const RETAINED_FILES: usize = 7;

/// `<local app data>/Arcana/logs` (C# log directory).
#[must_use]
pub fn default_log_dir() -> PathBuf {
    crate::settings::app_data_dir().join("logs")
}

/// The rolling file appender used by [`init_logging`].
///
/// # Errors
/// `FileAccessDenied` when the directory cannot be created.
pub fn rolling_appender(dir: &Path) -> AppResult<RollingFileAppender> {
    RollingFileAppender::builder()
        .rotation(Rotation::DAILY)
        .filename_prefix("app")
        .filename_suffix("log")
        .max_log_files(RETAINED_FILES)
        .build(dir)
        .map_err(|e| AppError::data(ErrorCode::FileAccessDenied, e.to_string()))
}

/// Install the global subscriber: stdout + rolling file under `dir`, level from `RUST_LOG`
/// (default `info`). Keep the returned guard alive for the app's lifetime so buffered lines
/// are flushed.
///
/// # Errors
/// `FileAccessDenied` when the log directory cannot be created; `Unknown` when a global
/// subscriber is already installed.
pub fn init_logging(dir: &Path) -> AppResult<WorkerGuard> {
    let (file, guard) = tracing_appender::non_blocking(rolling_appender(dir)?);
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .with_writer(std::io::stdout.and(file))
        .with_ansi(false)
        .try_init()
        .map_err(|e| AppError::Unknown {
            message: e.to_string(),
            inner: None,
        })?;
    Ok(guard)
}
