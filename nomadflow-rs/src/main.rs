mod attach;
mod daemon;
mod repo;

use std::path::PathBuf;

use clap::{Parser, Subcommand};
use color_eyre::Result;
use tokio_util::sync::CancellationToken;

use nomadflow_core::config::Settings;

#[derive(Parser)]
#[command(
    name = "nomadflow",
    version,
    about = "NomadFlow - Git worktree + PTY workflow manager"
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Show status and exit
    #[arg(long)]
    status: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Run the HTTP server in foreground
    Serve {
        /// Expose the server publicly via tunnel
        #[arg(long)]
        public: bool,
        /// Override the displayed address (IP or domain name) for QR code and URL
        #[arg(long)]
        host: Option<String>,
        /// Override the API server port (default: from config.toml)
        #[arg(long)]
        port: Option<u16>,
    },
    /// Start the server as a background daemon
    Start,
    /// Stop the background daemon
    Stop,
    /// Link an existing git repository
    Link {
        /// Path to the git repository
        path: PathBuf,
        /// Custom name for the link (defaults to directory name)
        #[arg(long)]
        name: Option<String>,
    },
    /// Unlink a previously linked repository
    Unlink {
        /// Name of the linked repository to remove
        name: Option<String>,
    },
    /// Open the web dashboard in a browser
    Web {
        /// Override the dashboard port (default: from config.toml)
        #[arg(long)]
        port: Option<u16>,
    },
    /// Attach to a server-managed pane via Unix socket
    Attach {
        /// Pane ID to attach to. If omitted, shows a picker.
        #[arg(long)]
        pane: Option<u16>,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;

    let cli = Cli::parse();
    let settings = Settings::load(None).unwrap_or_default();
    settings.ensure_directories()?;

    match cli.command {
        Some(Commands::Serve { public, host, port }) => {
            let mut settings = if !settings.config_file().exists() {
                match nomadflow_tui::run_setup(settings)? {
                    Some(s) => s,
                    None => return Ok(()),
                }
            } else {
                settings
            };
            if let Some(p) = port {
                settings.api.port = p;
            }
            nomadflow_server::init_tracing();
            let shutdown = CancellationToken::new();
            nomadflow_server::spawn_signal_handler(shutdown.clone());
            nomadflow_server::serve(settings, shutdown, public, false, host).await?;
        }
        Some(Commands::Start) => {
            daemon::start_daemon(&settings)?;
        }
        Some(Commands::Stop) => {
            daemon::stop_daemon(&settings)?;
        }
        Some(Commands::Link { path, name }) => {
            repo::link_repo(&settings, &path, name.as_deref())?;
        }
        Some(Commands::Unlink { name }) => {
            repo::unlink_repo(&settings, name.as_deref())?;
        }
        Some(Commands::Web { port }) => {
            let mut settings = settings;
            if let Some(p) = port {
                settings.web.port = p;
            }
            nomadflow_server::init_tracing();
            let shutdown = CancellationToken::new();
            nomadflow_server::spawn_signal_handler(shutdown.clone());
            nomadflow_server::serve_web(&settings, shutdown).await?;
        }
        Some(Commands::Attach { pane }) => {
            let socket_path = settings.socket_path();
            attach::run(&socket_path, pane).await?;
        }
        None if cli.status => {
            daemon::show_status(&settings);
            nomadflow_tui::run_status(&settings);
        }
        None => {
            // Default: run TUI wizard (server must be started manually with `nomadflow serve`)
            nomadflow_tui::run_tui(settings).await?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_parse_serve() {
        let cli = Cli::try_parse_from(["nomadflow", "serve"]).unwrap();
        assert!(matches!(cli.command, Some(Commands::Serve { .. })));
    }

    #[test]
    fn test_cli_parse_status() {
        let cli = Cli::try_parse_from(["nomadflow", "--status"]).unwrap();
        assert!(cli.status);
    }
}
