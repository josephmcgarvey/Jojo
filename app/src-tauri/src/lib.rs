//! Jojo app backend: daemon management, the connection to jojod, and the
//! bridge to the frontend.

pub mod bridge;
pub mod connection;
pub mod daemon_manager;
pub mod layout;
pub mod links;
pub mod settings;
pub mod shells;
#[cfg(windows)]
pub mod snap;
