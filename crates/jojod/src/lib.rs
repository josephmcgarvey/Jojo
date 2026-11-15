//! jojod: the Jojo session daemon.

pub mod askpass;
pub mod attach;
pub mod client;
pub mod client_queue;
pub mod command;
pub mod daemon;
pub mod hooks;
pub mod logging;
pub mod paths;
#[cfg(windows)]
mod pipe_security;
pub mod server;
pub mod session;
pub mod store;
pub mod transport;
#[cfg(unix)]
pub mod unix;

/// Daemon version reported in `Hello` and by `jojod --version`.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Scrollback lines kept per session (spec section 5.2).
pub const DEFAULT_SCROLLBACK: usize = 10_000;

/// True when this process runs elevated (Windows only).
#[cfg(windows)]
pub fn pipe_security_is_elevated() -> std::io::Result<bool> {
    pipe_security::is_elevated()
}
