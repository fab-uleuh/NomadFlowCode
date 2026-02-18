mod daemon;
mod repo;

use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;

use clap::{Parser, Subcommand};
use color_eyre::{eyre::eyre, Result};
use tokio_util::sync::CancellationToken;

use nomadflow_core::agent_state::AgentStateService;
use nomadflow_core::config::Settings;
use nomadflow_core::services::git::GitService;
use nomadflow_core::services::tmux::{session_window_name, TmuxService};

#[derive(Parser)]
#[command(
    name = "nomadflow",
    version,
    about = "NomadFlow - Git worktree + tmux workflow manager"
)]
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
    /// Create a managed terminal session in the current worktree
    Run {
        /// Agent type for state detection (claude-code uses hooks, generic uses process monitoring)
        #[arg(long, default_value = "generic", value_parser = ["generic", "claude-code"])]
        agent_type: String,
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
        Some(Commands::Attach { .. }) | Some(Commands::Run { .. }) => {
            ensure_dependencies(&["tmux"]).await;
        }
        _ => {}
    }

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
        Some(Commands::Run { agent_type }) => {
            let cwd = std::env::current_dir()
                .map_err(|e| eyre!("Failed to get current directory: {e}"))?;

            let git = GitService::new(&settings);
            let info = git
                .detect_current_worktree(&cwd)
                .await
                .map_err(|e| eyre!("{e}"))?;

            let tmux = TmuxService::new(&settings.tmux.session);
            tmux.ensure_session()
                .await
                .map_err(|e| eyre!("Failed to create tmux session: {e}"))?;

            let n = tmux
                .next_agent_number(&info.repo_name, &info.worktree_name)
                .await
                .map_err(|e| eyre!("Failed to determine agent number: {e}"))?;

            let window = session_window_name(&info.repo_name, &info.worktree_name, &agent_type, n);
            let session_id = window.replace(':', "-");

            // Set up agent state tracking hooks (only for claude-code)
            let agent_state = AgentStateService::new(&settings);
            let worktree_path = PathBuf::from(&info.worktree_path);

            if agent_type == "claude-code" {
                agent_state
                    .ensure_hook_scripts()
                    .await
                    .map_err(|e| eyre!("Failed to install hook scripts: {e}"))?;
                agent_state
                    .inject_hooks(&worktree_path, &session_id)
                    .await
                    .map_err(|e| eyre!("Failed to inject hooks: {e}"))?;
            }

            tmux.create_window(&window, Some(&info.worktree_path))
                .await
                .map_err(|e| eyre!("Failed to create tmux window: {e}"))?;

            // Set env vars in the tmux window regardless of agent type
            let state_dir = settings.sessions_dir().join(&session_id);
            let env_ok = tmux
                .send_keys(
                    &window,
                    &format!(
                        "export NOMADFLOW_SESSION_ID='{}' NOMADFLOW_STATE_DIR='{}'",
                        session_id,
                        state_dir.display()
                    ),
                    true,
                )
                .await;
            if !env_ok {
                eprintln!("Warning: Failed to set environment variables in tmux window");
            }

            println!("Session created: {window}");

            // Attach to the session, selecting the new window
            let session = tmux.session_name();
            let target = format!("{session}:{window}");
            std::process::Command::new("tmux")
                .args(["attach-session", "-t", &target])
                .stdin(Stdio::inherit())
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit())
                .status()?;

            // Cleanup hooks if the window was closed (only for claude-code)
            if agent_type == "claude-code" && !tmux.window_exists(&window).await {
                agent_state.cleanup_hooks(&worktree_path).await.ok();
            }
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

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn test_agent_type_defaults_to_generic() {
        let cli = Cli::try_parse_from(["nomadflow", "run"]).unwrap();
        match cli.command {
            Some(Commands::Run { agent_type }) => {
                assert_eq!(agent_type, "generic");
            }
            _ => panic!("Expected Run command"),
        }
    }

    #[test]
    fn test_agent_type_accepts_claude_code() {
        let cli = Cli::try_parse_from(["nomadflow", "run", "--agent-type", "claude-code"]).unwrap();
        match cli.command {
            Some(Commands::Run { agent_type }) => {
                assert_eq!(agent_type, "claude-code");
            }
            _ => panic!("Expected Run command"),
        }
    }

    #[test]
    fn test_agent_type_accepts_generic() {
        let cli = Cli::try_parse_from(["nomadflow", "run", "--agent-type", "generic"]).unwrap();
        match cli.command {
            Some(Commands::Run { agent_type }) => {
                assert_eq!(agent_type, "generic");
            }
            _ => panic!("Expected Run command"),
        }
    }

    #[test]
    fn test_agent_type_rejects_invalid_value() {
        let result = Cli::try_parse_from(["nomadflow", "run", "--agent-type", "invalid"]);
        assert!(result.is_err(), "Should reject invalid agent type values");
    }
}
