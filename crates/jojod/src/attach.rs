//! `jojod attach --stdio` (spec 3.2): the remote end of an SSH connection.
//!
//! The app runs `ssh <host> ~/.local/bin/jojod attach --stdio`. This command
//! connects to the daemon's socket and copies bytes in both directions:
//! stdin to the socket and the socket to stdout. It does not parse frames;
//! the app and the daemon speak the framed protocol through it unchanged.
//! It returns when either direction ends: SSH closing stdin (the app went
//! away or the connection dropped) or the daemon closing the socket.

use crate::paths::Endpoint;
use tokio::io::{AsyncWriteExt, BufWriter};

/// Relays between this process's stdio and the daemon at `endpoint`.
pub async fn relay_stdio(endpoint: &Endpoint) -> std::io::Result<()> {
    let (mut from_daemon, mut to_daemon) = crate::transport::connect(endpoint).await?;
    let mut stdin = tokio::io::stdin();
    let mut stdout = BufWriter::new(tokio::io::stdout());
    let upstream = async {
        tokio::io::copy(&mut stdin, &mut to_daemon).await?;
        to_daemon.shutdown().await
    };
    let downstream = async {
        // copy_buf would wait for a full buffer; copy flushes after each
        // read, so output reaches SSH as soon as the daemon sends it.
        let mut buf = vec![0u8; 64 * 1024];
        loop {
            let n = tokio::io::AsyncReadExt::read(&mut from_daemon, &mut buf).await?;
            if n == 0 {
                return Ok::<(), std::io::Error>(());
            }
            stdout.write_all(&buf[..n]).await?;
            stdout.flush().await?;
        }
    };
    tokio::select! {
        result = upstream => result,
        result = downstream => result,
    }
}
