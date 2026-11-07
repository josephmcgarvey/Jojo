//! File logging (spec section 10). Logs carry metadata only: session ids,
//! states, connection events and errors. Terminal input and output bytes are
//! never logged.

use std::path::Path;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_appender::rolling::{RollingFileAppender, Rotation};

/// Writes `jojod.<date>.log` files into `<state_dir>/logs`, rotated daily,
/// keeping the newest 7. Keep the returned guard alive for the life of the
/// process; dropping it flushes and stops the background writer.
pub fn init(state_dir: &Path) -> anyhow::Result<WorkerGuard> {
    // On Unix, create `state_dir` itself with mode 0700 before `logs/` goes
    // into it, so a daemon whose state directory did not already exist never
    // has a window where it is world- or group-readable. On Windows, ACLs on
    // the pipe (see `pipe_security.rs`) are the access-control mechanism
    // instead, so this is a no-op there.
    #[cfg(unix)]
    create_private_dir(state_dir)?;
    let log_dir = state_dir.join("logs");
    std::fs::create_dir_all(&log_dir)?;
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

/// Creates `dir` (and any missing parents) with mode 0700, and forces that
/// mode even when `dir` already existed with something looser: `DirBuilder`
/// only applies `mode` to a directory it actually creates, so a directory
/// created earlier by different code (or a different umask) would otherwise
/// keep whatever permissions it already had.
#[cfg(unix)]
fn create_private_dir(dir: &Path) -> anyhow::Result<()> {
    use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
    std::fs::DirBuilder::new().recursive(true).mode(0o700).create(dir)?;
    std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))?;
    Ok(())
}

// `init` itself is not unit-tested: it calls `tracing_subscriber::fmt()...
// try_init()`, which installs a process-global subscriber and errors if one
// is already installed, so a second test in the same process would fail
// regardless of this change. `create_private_dir` carries the behavior this
// fix adds, so it is tested directly here instead.
#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::{DirBuilderExt, PermissionsExt};

    fn mode_of(path: &Path) -> u32 {
        std::fs::metadata(path).unwrap().permissions().mode() & 0o777
    }

    #[test]
    fn creates_a_missing_directory_as_0700() {
        let base = tempfile::tempdir().unwrap();
        let state_dir = base.path().join("state");
        create_private_dir(&state_dir).unwrap();
        assert_eq!(mode_of(&state_dir), 0o700);
    }

    #[test]
    fn tightens_an_existing_looser_directory_to_0700() {
        let base = tempfile::tempdir().unwrap();
        let state_dir = base.path().join("state");
        std::fs::DirBuilder::new().mode(0o755).create(&state_dir).unwrap();
        assert_eq!(mode_of(&state_dir), 0o755);
        create_private_dir(&state_dir).unwrap();
        assert_eq!(mode_of(&state_dir), 0o700);
    }
}
