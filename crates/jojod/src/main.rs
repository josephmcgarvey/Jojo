use clap::{CommandFactory, Parser, Subcommand};
use jojo_proto::{ControlMsg, Frame, PROTOCOL_VERSION};
use jojod::client::Client;
use jojod::daemon::DaemonConfig;
use jojod::paths::{default_endpoint, default_state_dir, Endpoint};
use std::path::PathBuf;

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
            let cfg = DaemonConfig {
                endpoint: endpoint.map(Endpoint).unwrap_or_else(default_endpoint),
                state_dir: state_dir.unwrap_or_else(default_state_dir),
                scrollback: jojod::DEFAULT_SCROLLBACK,
            };
            jojod::server::run(cfg).await
        }
        Some(Command::Shutdown { endpoint }) => {
            let endpoint = endpoint.map(Endpoint).unwrap_or_else(default_endpoint);
            let mut client = Client::connect(&endpoint).await?;
            client.send(Frame::Control(ControlMsg::Shutdown)).await?;
            Ok(())
        }
        None => {
            Cli::command().print_help()?;
            Ok(())
        }
    }
}
