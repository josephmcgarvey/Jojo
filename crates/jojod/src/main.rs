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
        /// Linux: detach from the terminal and the SSH session and run in the
        /// background; returns once the daemon accepts connections.
        #[arg(long)]
        daemonize: bool,
    },
    /// Relay stdin and stdout to the running daemon (the remote end of the
    /// app's SSH connection).
    Attach {
        #[arg(long)]
        endpoint: Option<String>,
        /// Required; the only mode.
        #[arg(long)]
        stdio: bool,
    },
    /// Ask the running daemon to end all sessions and exit.
    Shutdown {
        #[arg(long)]
        endpoint: Option<String>,
    },
    /// Hook relay run by Claude Code: forwards the hook JSON on stdin to the
    /// daemon for the session in JOJO_SESSION_ID. Prints nothing; exits 0.
    Hook {
        #[arg(long)]
        endpoint: Option<String>,
    },
    /// Register Jojo's hooks in Claude Code's settings.json.
    InstallHooks {
        /// Defaults to ~/.claude/settings.json.
        #[arg(long)]
        settings: Option<PathBuf>,
        /// The jojod the hooks run. Defaults to this executable.
        #[arg(long)]
        command: Option<PathBuf>,
    },
    /// Remove Jojo's hooks from Claude Code's settings.json.
    UninstallHooks {
        #[arg(long)]
        settings: Option<PathBuf>,
    },
}

fn main() -> anyhow::Result<()> {
    // As ssh's password helper, jojod is run as `jojod "<prompt>"`, which is
    // not a subcommand; the app marks those runs with JOJO_SSH_ASKPASS.
    if std::env::var_os(jojod::askpass::ASKPASS_VAR).is_some() {
        std::process::exit(jojod::askpass::run());
    }
    let cli = Cli::parse();
    // The hook relay runs for every Claude Code hook event, also outside
    // Jojo. Spec 7.1: return at once when JOJO_SESSION_ID is unset, before
    // building any runtime; otherwise one thread is enough to send one frame.
    if let Some(Command::Hook { endpoint }) = cli.command {
        let Ok(session) = std::env::var("JOJO_SESSION_ID") else { return Ok(()) };
        let mut payload = String::new();
        if std::io::Read::read_to_string(&mut std::io::stdin(), &mut payload).is_err() {
            return Ok(());
        }
        let endpoint = endpoint.map(Endpoint).unwrap_or_else(default_endpoint);
        if let Ok(runtime) = tokio::runtime::Builder::new_current_thread().enable_all().build() {
            runtime.block_on(jojod::hooks::relay(&endpoint, Some(&session), payload));
        }
        return Ok(());
    }
    #[cfg(unix)]
    let _lock = match &cli.command {
        Some(Command::Serve { endpoint, state_dir, daemonize }) => {
            let state_dir = state_dir.clone().unwrap_or_else(default_state_dir);
            if *daemonize {
                let socket = endpoint.clone().map(Endpoint).unwrap_or_else(default_endpoint);
                match jojod::unix::daemonize(std::path::Path::new(&socket.0))? {
                    jojod::unix::Role::Parent(ready) => return ready.map_err(Into::into),
                    jojod::unix::Role::Daemon => {}
                }
            }
            // Spec 8.2 step 5: a second `serve` finds the lock held and exits
            // cleanly; the running daemon keeps its sessions.
            match jojod::unix::lock_state_dir(&state_dir)? {
                Some(lock) => Some(lock),
                None => {
                    eprintln!("jojod is already running for {}", state_dir.display());
                    return Ok(());
                }
            }
        }
        _ => None,
    };
    tokio::runtime::Builder::new_multi_thread().enable_all().build()?.block_on(run(cli))
}

async fn run(cli: Cli) -> anyhow::Result<()> {
    if cli.version {
        println!("jojod {} (protocol {})", jojod::VERSION, PROTOCOL_VERSION);
        return Ok(());
    }
    match cli.command {
        Some(Command::Serve { endpoint, state_dir, daemonize }) => {
            #[cfg(windows)]
            if daemonize {
                anyhow::bail!("--daemonize is for Linux; the app starts jojod detached on Windows");
            }
            #[cfg(unix)]
            let _ = daemonize;
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
        // Handled in `main` before a multi-thread runtime exists.
        Some(Command::Hook { .. }) => Ok(()),
        Some(Command::Attach { endpoint, stdio }) => {
            if !stdio {
                anyhow::bail!("attach needs --stdio");
            }
            let endpoint = endpoint.map(Endpoint).unwrap_or_else(default_endpoint);
            let result = jojod::attach::relay_stdio(&endpoint).await;
            if let Err(e) = &result {
                eprintln!("Error: {e}");
            }
            // tokio reads stdin with a blocking read on its blocking pool,
            // which cannot be cancelled; returning would drop the runtime,
            // which waits for that read, so stdout (and the SSH channel)
            // would stay open after the daemon closed the socket and the app
            // would not notice until the next keystroke. Exit at once.
            std::process::exit(if result.is_ok() { 0 } else { 1 });
        }
        Some(Command::InstallHooks { settings, command }) => {
            let settings = settings.or_else(jojod::hooks::default_settings_path).ok_or_else(|| anyhow::anyhow!("no home directory"))?;
            let jojod = match command {
                Some(path) => path,
                None => std::env::current_exe()?,
            };
            let changed = jojod::hooks::install(&settings, &jojod::hooks::hook_command(&jojod))?;
            println!("{} {}", if changed { "Registered Jojo's hooks in" } else { "Jojo's hooks are already registered in" }, settings.display());
            Ok(())
        }
        Some(Command::UninstallHooks { settings }) => {
            let settings = settings.or_else(jojod::hooks::default_settings_path).ok_or_else(|| anyhow::anyhow!("no home directory"))?;
            let changed = jojod::hooks::uninstall(&settings)?;
            println!("{} {}", if changed { "Removed Jojo's hooks from" } else { "No Jojo hooks in" }, settings.display());
            Ok(())
        }
        None => {
            Cli::command().print_help()?;
            Ok(())
        }
    }
}
