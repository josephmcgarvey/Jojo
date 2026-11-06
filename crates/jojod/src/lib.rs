//! jojod: the Jojo session daemon.

pub mod command;
pub mod paths;
pub mod session;
pub mod store;

/// Daemon version reported in `Hello` and by `jojod --version`.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Scrollback lines kept per session (spec section 5.2).
pub const DEFAULT_SCROLLBACK: usize = 10_000;
