//! Byte-stream transport between the daemon and its clients: a named pipe
//! on Windows, a Unix socket on Linux (spec section 3.1).

use crate::paths::Endpoint;
use std::io;
use tokio::io::{AsyncRead, AsyncWrite};

pub type BoxRead = Box<dyn AsyncRead + Unpin + Send>;
pub type BoxWrite = Box<dyn AsyncWrite + Unpin + Send>;

#[cfg(windows)]
mod imp {
    use super::*;
    use crate::pipe_security::UserOnly;
    use std::time::Duration;
    use tokio::net::windows::named_pipe::{ClientOptions, NamedPipeServer, ServerOptions};

    const ERROR_PIPE_BUSY: i32 = 231;

    pub struct Listener {
        name: String,
        next: NamedPipeServer,
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
            Ok(Listener { name: endpoint.0.clone(), next: create(&endpoint.0, true)? })
        }

        pub async fn accept(&mut self) -> io::Result<(BoxRead, BoxWrite)> {
            self.next.connect().await?;
            let fresh = create(&self.name, false)?;
            let connected = std::mem::replace(&mut self.next, fresh);
            let (r, w) = tokio::io::split(connected);
            Ok((Box::new(r), Box::new(w)))
        }
    }

    pub async fn connect(endpoint: &Endpoint) -> io::Result<(BoxRead, BoxWrite)> {
        loop {
            match ClientOptions::new().open(&endpoint.0) {
                Ok(client) => {
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

    pub async fn connect(endpoint: &Endpoint) -> io::Result<(BoxRead, BoxWrite)> {
        let (r, w) = UnixStream::connect(&endpoint.0).await?.into_split();
        Ok((Box::new(r), Box::new(w)))
    }
}

pub use imp::{connect, Listener};
