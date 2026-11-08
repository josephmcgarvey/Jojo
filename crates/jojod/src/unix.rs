//! Linux daemon startup (spec 8.2 step 5): the state directory lock and
//! `serve --daemonize`.
//!
//! Both run before the tokio runtime exists: `fork` in a process with more
//! than one thread copies only the calling thread, so a runtime's worker
//! threads and any locks they hold would be lost in the child.

use std::fs::File;
use std::io;
use std::os::fd::AsRawFd;
use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
use std::path::Path;
use std::time::{Duration, Instant};

/// How long `serve --daemonize` waits for the daemon's socket to accept a
/// connection before reporting failure.
const READY_TIMEOUT: Duration = Duration::from_secs(5);

/// Creates `state_dir` with mode 0700 and takes an exclusive `flock` on
/// `state_dir/jojod.lock`. Returns `Ok(None)` when another jojod holds it;
/// the returned file keeps the lock for as long as it is open.
pub fn lock_state_dir(state_dir: &Path) -> io::Result<Option<File>> {
    std::fs::DirBuilder::new().recursive(true).mode(0o700).create(state_dir)?;
    std::fs::set_permissions(state_dir, std::fs::Permissions::from_mode(0o700))?;
    let file = std::fs::OpenOptions::new().create(true).truncate(false).write(true).open(state_dir.join("jojod.lock"))?;
    // SAFETY: flock on a file descriptor this function owns.
    let rc = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
    if rc == 0 {
        return Ok(Some(file));
    }
    let err = io::Error::last_os_error();
    if err.raw_os_error() == Some(libc::EWOULDBLOCK) {
        Ok(None)
    } else {
        Err(err)
    }
}

/// What the calling process should do after `daemonize`.
pub enum Role {
    /// The original process: the daemon's socket accepted a connection
    /// (`Ok`) or did not within `READY_TIMEOUT` (`Err`). Exit afterwards.
    Parent(io::Result<()>),
    /// The detached daemon process: continue starting the daemon.
    Daemon,
}

/// Detaches from the terminal and the SSH session: forks, starts a new
/// session with `setsid` in the child, forks again so the daemon is not a
/// session leader (it can never acquire a controlling terminal), points
/// stdin, stdout and stderr at /dev/null, and changes to `/`. The original
/// process waits until a connection to `socket` succeeds, so the caller
/// (the app's install step) can connect right after `serve --daemonize`
/// returns. A daemon that finds another one holding the lock exits, and the
/// running one answers the connection, so a second `serve --daemonize` also
/// succeeds.
pub fn daemonize(socket: &Path) -> io::Result<Role> {
    // SAFETY: called before any other thread exists (see the module docs).
    match unsafe { libc::fork() } {
        -1 => return Err(io::Error::last_os_error()),
        0 => {}
        _ => return Ok(Role::Parent(wait_until_ready(socket))),
    }
    // SAFETY: plain libc calls in the single-threaded child.
    unsafe {
        if libc::setsid() == -1 {
            libc::_exit(1);
        }
        match libc::fork() {
            -1 => libc::_exit(1),
            0 => {}
            _ => libc::_exit(0),
        }
        let null = libc::open(c"/dev/null".as_ptr(), libc::O_RDWR);
        if null >= 0 {
            for fd in 0..=2 {
                libc::dup2(null, fd);
            }
            if null > 2 {
                libc::close(null);
            }
        }
        libc::chdir(c"/".as_ptr());
    }
    Ok(Role::Daemon)
}

fn wait_until_ready(socket: &Path) -> io::Result<()> {
    let deadline = Instant::now() + READY_TIMEOUT;
    loop {
        if std::os::unix::net::UnixStream::connect(socket).is_ok() {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err(io::Error::new(io::ErrorKind::TimedOut, format!("jojod did not start listening on {}", socket.display())));
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_second_lock_on_the_same_directory_is_refused_until_the_first_is_dropped() {
        let dir = tempfile::tempdir().unwrap();
        let state = dir.path().join("state");
        let first = lock_state_dir(&state).unwrap().expect("first lock");
        // flock locks belong to the open file description, so a second open
        // in the same process conflicts exactly as another process would.
        assert!(lock_state_dir(&state).unwrap().is_none());
        drop(first);
        assert!(lock_state_dir(&state).unwrap().is_some());
        let mode = std::fs::metadata(&state).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o700);
    }
}
