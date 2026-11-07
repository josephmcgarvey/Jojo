//! File logging (spec section 10). Logs carry metadata only: session ids,
//! states, connection events and errors. Terminal input and output bytes are
//! never logged.

use std::path::Path;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_appender::rolling::{RollingFileAppender, Rotation};

/// Writes `jojod.<date>.log` files into `log_dir`, rotated daily, keeping
/// the newest 7. Keep the returned guard alive for the life of the process;
/// dropping it flushes and stops the background writer.
pub fn init(log_dir: &Path) -> anyhow::Result<WorkerGuard> {
    std::fs::create_dir_all(log_dir)?;
    let appender = RollingFileAppender::builder()
        .rotation(Rotation::DAILY)
        .filename_prefix("jojod")
        .filename_suffix("log")
        .max_log_files(7)
        .build(log_dir)?;
    let (writer, guard) = tracing_appender::non_blocking(appender);
    tracing_subscriber::fmt()
        .with_writer(writer)
        .with_ansi(false)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_env("JOJO_LOG").unwrap_or_else(|_| "info".into()),
        )
        .try_init()
        .map_err(|e| anyhow::anyhow!("install log subscriber: {e}"))?;
    Ok(guard)
}
