mod daemon;
mod repo;

use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;

use clap::{Parser, Subcommand};
use color_eyre::{eyre::eyre, Result};
use tokio_util::sync::CancellationToken;

use nomadflow_core::config::Settings;

#[derive(Parser)]
#[command(name = "nomadflow", version, about = "NomadFlow - Git worktree + tmux workflow manager")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Show tmux status and exit
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
    /// Attach to an existing tmux window (no server needed)
    Attach {
        /// Window name (e.g. "omstudio:my-feature"). If omitted, shows a picker.
        window: Option<String>,
    },
}

async fn ensure_dependencies(required: &[&str]) {
    let mut missing = Vec::new();
    for &tool in required {
        if !nomadflow_core::shell::command_exists(tool).await {
            missing.push(tool);
        }
    }
    if missing.is_empty() {
        return;
    }

    let tools = missing.join(" and ");
    eprintln!();
    eprintln!("  ERROR: {tools} not found");
    eprintln!();
    eprintln!("  NomadFlow requires {tools} to manage terminal sessions.");
    eprintln!();
    eprintln!("  Install instructions:");
    eprintln!();
    for tool in &missing {
        eprintln!("    {tool}:");
        if cfg!(target_os = "macos") {
            eprintln!("      brew install {tool}");
        } else {
            eprintln!("      sudo apt install {tool}    # Debian/Ubuntu");
            eprintln!("      sudo dnf install {tool}    # Fedora/RHEL");
        }
        eprintln!();
    }
    std::process::exit(1);
}

fn attach_local(settings: &Settings, window: Option<String>) -> Result<()> {
    let session = &settings.tmux.session;

    if !nomadflow_tui::tmux_local::session_exists(session) {
        return Err(eyre!(
            "No tmux session '{session}' found. Start one with `nomadflow` first."
        ));
    }

    if let Some(w) = window {
        nomadflow_tui::tmux_local::attach_session_target(session, Some(&w));
        return Ok(());
    }

    let windows = nomadflow_tui::tmux_local::list_windows(session);

    if windows.is_empty() {
        return Err(eyre!("Session '{session}' has no windows."));
    }

    if windows.len() == 1 {
        nomadflow_tui::tmux_local::attach_session_target(session, Some(&windows[0].name));
        return Ok(());
    }

    // Multiple windows → show picker
    let items: Vec<nomadflow_tui::PickItem> = windows
        .iter()
        .map(|w| {
            let cmd = nomadflow_tui::tmux_local::get_pane_command(session, &w.name);
            let idle = nomadflow_tui::tmux_local::is_shell_idle_str(cmd.as_deref());
            let detail = match &cmd {
                Some(c) if !idle => c.clone(),
                _ => "idle".to_string(),
            };
            nomadflow_tui::PickItem {
                label: w.name.clone(),
                detail,
            }
        })
        .collect();

    match nomadflow_tui::pick_from_list("Attach to window:", &items)? {
        Some(idx) => {
            nomadflow_tui::tmux_local::attach_session_target(session, Some(&windows[idx].name));
        }
        None => {} // cancelled
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;

    let cli = Cli::parse();
    let settings = Settings::load(None).unwrap_or_default();
    settings.ensure_directories()?;

    match &cli.command {
        Some(Commands::Serve { .. }) | Some(Commands::Start) | None if !cli.status => {
            ensure_dependencies(&["tmux", "ttyd"]).await;
        }
        Some(Commands::Attach { .. }) => {
            ensure_dependencies(&["tmux"]).await;
        }
        _ => {}
    }

    match cli.command {
        Some(Commands::Serve { public, host }) => {
            let settings = if !settings.config_file().exists() {
                match nomadflow_tui::run_setup(settings)? {
                    Some(s) => s,
                    None => return Ok(()),
                }
            } else {
                settings
            };
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
        Some(Commands::Attach { window }) => {
            attach_local(&settings, window)?;
        }
        None if cli.status => {
            daemon::show_status(&settings);
            nomadflow_tui::run_status(&settings);
        }
        None => {
            // Default: spawn server in background + TUI wizard
            let server_settings = settings.clone();
            let shutdown = CancellationToken::new();
            let shutdown_clone = shutdown.clone();
            let server_handle = tokio::spawn(async move {
                nomadflow_server::serve(server_settings, shutdown_clone, false, true, None)
                    .await
                    .ok();
            });

            // Run TUI
            let attach_session = nomadflow_tui::run_tui(settings).await?;

            // Graceful shutdown instead of abort
            shutdown.cancel();
            tokio::select! {
                _ = server_handle => {}
                _ = tokio::time::sleep(Duration::from_secs(5)) => {}
            }

            // Attach to tmux if TUI returned a session
            if let Some(session) = attach_session {
                std::process::Command::new("tmux")
                    .args(["attach-session", "-t", &session])
                    .stdin(Stdio::inherit())
                    .stdout(Stdio::inherit())
                    .stderr(Stdio::inherit())
                    .status()?;
            }
        }
    }

    Ok(())
}
