//! Byte-stream transport between the daemon and its clients: a named pipe
//! on Windows, a Unix socket on Linux (spec section 3.1).

use crate::paths::Endpoint;
use std::future::Future;
use std::io;
use tokio::io::{AsyncRead, AsyncWrite};

pub type BoxRead = Box<dyn AsyncRead + Unpin + Send>;
pub type BoxWrite = Box<dyn AsyncWrite + Unpin + Send>;

#[cfg(windows)]
mod imp {
    use super::*;
    use crate::pipe_security::UserOnly;
    use std::os::windows::io::AsRawHandle;
    use std::time::Duration;
    use tokio::net::windows::named_pipe::{ClientOptions, NamedPipeServer, ServerOptions};

    const ERROR_PIPE_BUSY: i32 = 231;

    pub struct Listener {
        name: String,
        // `None` means the next instance still needs to be created: either
        // this is the first call, or a previous call consumed its instance
        // (a successful accept) or left it unusable (`connect` failed, or
        // `create` for the following instance failed). `accept` never
        // reuses a stale or already-consumed handle; it creates a fresh one
        // whenever this is `None` (Finding F1).
        next: Option<NamedPipeServer>,
    }

    fn create(name: &str, first: bool) -> io::Result<NamedPipeServer> {
        let mut security = UserOnly::new()?;
        let mut options = ServerOptions::new();
        // first_pipe_instance(true) fails if another daemon already owns the
        // name, which makes the daemon single-instance per user.
        options.first_pipe_instance(first).reject_remote_clients(true);
        // SAFETY: the security attributes outlive the create call.
        unsafe { options.create_with_security_attributes_raw(name, security.as_ptr()) }
    }

    impl Listener {
        pub fn bind(endpoint: &Endpoint) -> io::Result<Listener> {
            Ok(Listener { name: endpoint.0.clone(), next: Some(create(&endpoint.0, true)?) })
        }

        pub async fn accept(&mut self) -> io::Result<(BoxRead, BoxWrite)> {
            let server = match self.next.take() {
                Some(s) => s,
                None => create(&self.name, false)?,
            };
            server.connect().await?;
            // Prepare the instance for the *next* connection. If this fails
            // (for example under resource exhaustion), the client we just
            // accepted is still returned below; `next` stays `None` and the
            // following `accept()` call retries `create` instead of reusing
            // a consumed instance.
            self.next = create(&self.name, false).ok();
            let (r, w) = tokio::io::split(server);
            Ok((Box::new(r), Box::new(w)))
        }
    }

    pub(super) async fn connect_unbounded(endpoint: &Endpoint) -> io::Result<(BoxRead, BoxWrite)> {
        loop {
            match ClientOptions::new().open(&endpoint.0) {
                Ok(client) => {
                    crate::pipe_security::verify_server_is_current_user(
                        client.as_raw_handle() as windows_sys::Win32::Foundation::HANDLE
                    )?;
                    let (r, w) = tokio::io::split(client);
                    return Ok((Box::new(r), Box::new(w)));
                }
                Err(e) if e.raw_os_error() == Some(ERROR_PIPE_BUSY) => {
                    tokio::time::sleep(Duration::from_millis(50)).await;
                }
                Err(e) => return Err(e),
            }
        }
    }
}

#[cfg(unix)]
mod imp {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    use std::path::Path;
    use tokio::net::{UnixListener, UnixStream};

    pub struct Listener {
        inner: UnixListener,
    }

    impl Listener {
        pub fn bind(endpoint: &Endpoint) -> io::Result<Listener> {
            let path = Path::new(&endpoint.0);
            if path.exists() {
                if std::os::unix::net::UnixStream::connect(path).is_ok() {
                    return Err(io::Error::new(io::ErrorKind::AddrInUse, "jojod is already running"));
                }
                std::fs::remove_file(path)?;
            }
            if let Some(dir) = path.parent() {
                std::fs::create_dir_all(dir)?;
                std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))?;
            }
            let inner = UnixListener::bind(path)?;
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
            Ok(Listener { inner })
        }

        pub async fn accept(&mut self) -> io::Result<(BoxRead, BoxWrite)> {
            let (stream, _) = self.inner.accept().await?;
            let (r, w) = stream.into_split();
            Ok((Box::new(r), Box::new(w)))
        }
    }

    pub(super) async fn connect_unbounded(endpoint: &Endpoint) -> io::Result<(BoxRead, BoxWrite)> {
        let (r, w) = UnixStream::connect(&endpoint.0).await?.into_split();
        Ok((Box::new(r), Box::new(w)))
    }
}

pub use imp::Listener;

/// How long `connect` keeps retrying a busy pipe before giving up.
pub const CONNECT_DEADLINE: std::time::Duration = std::time::Duration::from_secs(5);

pub async fn connect(endpoint: &Endpoint) -> io::Result<(BoxRead, BoxWrite)> {
    connect_with_deadline(endpoint, CONNECT_DEADLINE).await
}

pub async fn connect_with_deadline(
    endpoint: &Endpoint,
    deadline: std::time::Duration,
) -> io::Result<(BoxRead, BoxWrite)> {
    tokio::time::timeout(deadline, imp::connect_unbounded(endpoint))
        .await
        .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "timed out connecting to jojod"))?
}

/// Abstracts the accept loop over the real transport so `server::run` can be
/// tested against a fake that injects accept errors deterministically
/// (Finding F1). Implemented by `Listener` on every platform.
pub trait Accept: Send {
    fn accept(&mut self) -> impl Future<Output = io::Result<(BoxRead, BoxWrite)>> + Send;
}

impl Accept for Listener {
    async fn accept(&mut self) -> io::Result<(BoxRead, BoxWrite)> {
        Listener::accept(self).await
    }
}
