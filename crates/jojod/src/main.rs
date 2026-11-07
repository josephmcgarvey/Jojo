use clap::{CommandFactory, Parser, Subcommand};
use jojo_proto::{ControlMsg, Frame, PROTOCOL_VERSION};
use jojod::client::Client;
use jojod::daemon::DaemonConfig;
use jojod::paths::{default_endpoint, default_state_dir, Endpoint};
use std::path::PathBuf;
use std::time::Duration;

/// Bounds how long `jojod shutdown` waits for the daemon to actually exit
/// (I7) after the `Shutdown` frame is acknowledged by the OS transport.
const SHUTDOWN_WAIT: Duration = Duration::from_secs(5);

#[derive(Parser)]
#[command(name = "jojod", about = "Jojo session daemon", disable_version_flag = true)]
struct Cli {
    /// Print the daemon version and protocol version.
    #[arg(short = 'V', long = "version")]
    version: bool,
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Run the daemon in the foreground.
    Serve {
        /// Pipe name (Windows) or socket path (Linux). Defaults to the per-user endpoint.
        #[arg(long)]
        endpoint: Option<String>,
        /// Directory for state.json. Defaults to %LOCALAPPDATA%\Jojo or ~/.jojo.
        #[arg(long)]
        state_dir: Option<PathBuf>,
    },
    /// Ask the running daemon to end all sessions and exit.
    Shutdown {
        #[arg(long)]
        endpoint: Option<String>,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    if cli.version {
        println!("jojod {} (protocol {})", jojod::VERSION, PROTOCOL_VERSION);
        return Ok(());
    }
    match cli.command {
        Some(Command::Serve { endpoint, state_dir }) => {
            #[cfg(windows)]
            {
                use windows_sys::Win32::System::LibraryLoader::{
                    SetDefaultDllDirectories, LOAD_LIBRARY_SEARCH_APPLICATION_DIR, LOAD_LIBRARY_SEARCH_SYSTEM32,
                };
                // portable-pty loads "conpty.dll" by bare name. Restrict the search to
                // jojod's own folder and System32, so a missing sideloaded copy falls
                // back to the system ConPTY instead of a conpty.dll found on PATH or in
                // the current directory.
                // SAFETY: plain Win32 call with constant flags.
                let ok = unsafe { SetDefaultDllDirectories(LOAD_LIBRARY_SEARCH_APPLICATION_DIR | LOAD_LIBRARY_SEARCH_SYSTEM32) };
                if ok == 0 {
                    eprintln!("jojod: SetDefaultDllDirectories failed: {}", std::io::Error::last_os_error());
                }
            }
            #[cfg(windows)]
            if jojod::pipe_security_is_elevated()? {
                anyhow::bail!(
                    "jojod will not run elevated: its pipe would let non-elevated processes drive \
                     elevated shells. Start it from a terminal that is not running as administrator."
                );
            }
            let cfg = DaemonConfig {
                endpoint: endpoint.map(Endpoint).unwrap_or_else(default_endpoint),
                state_dir: state_dir.unwrap_or_else(default_state_dir),
                scrollback: jojod::DEFAULT_SCROLLBACK,
            };
            let _log_guard = jojod::logging::init(&cfg.state_dir)?;
            tracing::info!(version = jojod::VERSION, endpoint = %cfg.endpoint.0, "jojod starting");
            jojod::server::run(cfg).await
        }
        Some(Command::Shutdown { endpoint }) => {
            let endpoint = endpoint.map(Endpoint).unwrap_or_else(default_endpoint);
            let mut client = Client::connect(&endpoint).await?;
            client.send(Frame::Control(ControlMsg::Shutdown)).await?;
            // Sending the Shutdown frame only asks the daemon to start
            // exiting: `run_with_listener` still has to notice
            // `daemon.shutdown.notified()`, break its accept loop, call
            // `daemon.kill_all()`, and return before the process actually
            // exits and every connection (including this one) gets EOF.
            // Returning as soon as `send` completes would let a caller that
            // immediately runs `serve` on the same endpoint race the still-
            // exiting daemon and fail to bind it. Reading frames until EOF
            // (or any other read error, which also means the connection is
            // gone) makes this command synchronous with the daemon's actual
            // exit instead.
            let drained = tokio::time::timeout(SHUTDOWN_WAIT, async {
                loop {
                    if client.recv().await.is_err() {
                        return;
                    }
                }
            })
            .await;
            if drained.is_err() {
                eprintln!("jojod: timed out waiting for the daemon to exit");
                std::process::exit(1);
            }
            Ok(())
        }
        None => {
            Cli::command().print_help()?;
            Ok(())
        }
    }
}
