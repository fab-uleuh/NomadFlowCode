use std::path::{Path, PathBuf};
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

fn pid_file(settings: &Settings) -> PathBuf {
    settings.base_dir().join("nomadflow.pid")
}

fn log_file(settings: &Settings) -> PathBuf {
    settings.base_dir().join("nomadflow.log")
}

fn is_process_running(pid: u32) -> bool {
    std::process::Command::new("kill")
        .args(["-0", &pid.to_string()])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn start_daemon(settings: &Settings) -> Result<()> {
    let pid_path = pid_file(settings);

    // Check if already running
    if pid_path.exists() {
        if let Ok(contents) = std::fs::read_to_string(&pid_path) {
            if let Ok(pid) = contents.trim().parse::<u32>() {
                if is_process_running(pid) {
                    eprintln!("NomadFlow daemon already running (PID {pid})");
                    return Ok(());
                }
                // Stale PID file, remove it
                std::fs::remove_file(&pid_path)?;
            }
        }
    }

    let log_path = log_file(settings);
    let log = std::fs::File::create(&log_path)?;

    let exe = std::env::current_exe()?;
    let child = std::process::Command::new(exe)
        .arg("serve")
        .stdin(Stdio::null())
        .stdout(log.try_clone()?)
        .stderr(log)
        .spawn()?;

    let pid = child.id();
    std::fs::write(&pid_path, pid.to_string())?;

    eprintln!("NomadFlow daemon started (PID {pid})");
    eprintln!("Logs: {}", log_path.display());
    Ok(())
}

fn stop_daemon(settings: &Settings) -> Result<()> {
    let pid_path = pid_file(settings);

    if !pid_path.exists() {
        eprintln!("No PID file found — daemon not running");
        return Ok(());
    }

    let contents = std::fs::read_to_string(&pid_path)?;
    let pid: u32 = contents
        .trim()
        .parse()
        .map_err(|_| color_eyre::eyre::eyre!("Invalid PID file"))?;

    if !is_process_running(pid) {
        eprintln!("Process {pid} not running, removing stale PID file");
        std::fs::remove_file(&pid_path)?;
        return Ok(());
    }

    // Send SIGTERM
    eprintln!("Stopping NomadFlow daemon (PID {pid})…");
    std::process::Command::new("kill")
        .args([&pid.to_string()])
        .status()?;

    // Wait for process to exit (up to 10s)
    for _ in 0..20 {
        std::thread::sleep(Duration::from_millis(500));
        if !is_process_running(pid) {
            break;
        }
    }

    if is_process_running(pid) {
        eprintln!("Process did not exit, sending SIGKILL…");
        std::process::Command::new("kill")
            .args(["-9", &pid.to_string()])
            .status()?;
    }

    if pid_path.exists() {
        std::fs::remove_file(&pid_path)?;
    }

    eprintln!("NomadFlow daemon stopped");
    Ok(())
}

fn link_repo(settings: &Settings, path: &Path, name: Option<&str>) -> Result<()> {
    let canonical = path
        .canonicalize()
        .map_err(|_| eyre!("Path does not exist: {}", path.display()))?;

    if !canonical.join(".git").exists() {
        return Err(eyre!(
            "Not a git repository: {} (no .git directory)",
            canonical.display()
        ));
    }

    let link_name = match name {
        Some(n) => n.to_string(),
        None => canonical
            .file_name()
            .ok_or_else(|| eyre!("Cannot determine directory name from path"))?
            .to_string_lossy()
            .to_string(),
    };

    let repos_dir = settings.repos_dir();
    let link_path = repos_dir.join(&link_name);

    if link_path.exists() {
        return Err(eyre!(
            "A repository named '{}' already exists in {}",
            link_name,
            repos_dir.display()
        ));
    }

    std::os::unix::fs::symlink(&canonical, &link_path)?;
    eprintln!("Linked {} -> {}", link_name, canonical.display());
    Ok(())
}

fn unlink_repo(settings: &Settings, name: Option<&str>) -> Result<()> {
    let repos_dir = settings.repos_dir();

    // Collect all symlinks in repos_dir
    let mut links: Vec<(String, PathBuf)> = Vec::new();
    if repos_dir.exists() {
        for entry in std::fs::read_dir(&repos_dir)? {
            let entry = entry?;
            let meta = entry.path().symlink_metadata()?;
            if meta.is_symlink() {
                let link_name = entry.file_name().to_string_lossy().to_string();
                let target = std::fs::read_link(entry.path()).unwrap_or_default();
                links.push((link_name, target));
            }
        }
    }

    if links.is_empty() {
        eprintln!("No linked repositories found.");
        return Ok(());
    }

    let chosen = if let Some(n) = name {
        let found = links.iter().find(|(ln, _)| ln == n);
        match found {
            Some(entry) => entry.clone(),
            None => return Err(eyre!("No linked repository named '{n}'")),
        }
    } else {
        let items: Vec<nomadflow_tui::PickItem> = links
            .iter()
            .map(|(n, target)| nomadflow_tui::PickItem {
                label: n.clone(),
                detail: format!("-> {}", target.display()),
            })
            .collect();

        match nomadflow_tui::pick_from_list("Unlink a repository:", &items)? {
            Some(idx) => links[idx].clone(),
            None => return Ok(()), // cancelled
        }
    };

    let link_path = repos_dir.join(&chosen.0);

    // Safety: only remove symlinks, not real repos
    let meta = link_path.symlink_metadata()?;
    if !meta.is_symlink() {
        return Err(eyre!(
            "'{}' is not a symlink — refusing to remove a cloned repository",
            chosen.0
        ));
    }

    // Check for worktrees in ~/.nomadflowcode/worktrees/{repo_name}/
    let repo_worktrees_dir = settings.worktrees_dir().join(&chosen.0);
    if repo_worktrees_dir.exists() {
        let worktrees: Vec<_> = std::fs::read_dir(&repo_worktrees_dir)?
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_dir())
            .collect();

        if !worktrees.is_empty() {
            let wt_names: Vec<String> = worktrees
                .iter()
                .map(|wt| wt.file_name().to_string_lossy().to_string())
                .collect();
            let msg = format!(
                "Remove {} worktree(s)? ({})",
                worktrees.len(),
                wt_names.join(", ")
            );

            if nomadflow_tui::confirm(&msg)? {
                let repo_real_path = std::fs::read_link(&link_path)?;

                for wt in &worktrees {
                    let wt_path = wt.path();
                    let wt_str = wt_path.to_string_lossy();
                    let wt_name = wt.file_name().to_string_lossy().to_string();

                    let status = std::process::Command::new("git")
                        .args(["worktree", "remove", "--force", &wt_str])
                        .current_dir(&repo_real_path)
                        .stdout(Stdio::null())
                        .stderr(Stdio::null())
                        .status();

                    if status.map(|s| s.success()).unwrap_or(false) {
                        eprintln!("Removed worktree {wt_name}");
                    } else {
                        std::fs::remove_dir_all(&wt_path).ok();
                        eprintln!("Removed worktree directory {wt_name}");
                    }
                }

                std::process::Command::new("git")
                    .args(["worktree", "prune"])
                    .current_dir(&repo_real_path)
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .status()
                    .ok();

                std::fs::remove_dir(&repo_worktrees_dir).ok();
            }
        }
    }

    std::fs::remove_file(&link_path)?;
    eprintln!("Unlinked {}", chosen.0);
    Ok(())
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

fn show_daemon_status(settings: &Settings) {
    let pid_path = pid_file(settings);

    if pid_path.exists() {
        if let Ok(contents) = std::fs::read_to_string(&pid_path) {
            if let Ok(pid) = contents.trim().parse::<u32>() {
                if is_process_running(pid) {
                    eprintln!("NomadFlow daemon: running (PID {pid})");
                } else {
                    eprintln!("NomadFlow daemon: not running (stale PID file)");
                }
                return;
            }
        }
    }

    eprintln!("NomadFlow daemon: not running");
}

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;

    let cli = Cli::parse();
    let settings = Settings::load(None).unwrap_or_default();
    settings.ensure_directories()?;

    match cli.command {
        Some(Commands::Serve { public }) => {
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
            nomadflow_server::serve(settings, shutdown, public, false).await?;
        }
        Some(Commands::Start) => {
            start_daemon(&settings)?;
        }
        Some(Commands::Stop) => {
            stop_daemon(&settings)?;
        }
        Some(Commands::Link { path, name }) => {
            link_repo(&settings, &path, name.as_deref())?;
        }
        Some(Commands::Unlink { name }) => {
            unlink_repo(&settings, name.as_deref())?;
        }
        Some(Commands::Attach { window }) => {
            attach_local(&settings, window)?;
        }
        None if cli.status => {
            show_daemon_status(&settings);
            nomadflow_tui::run_status(&settings);
        }
        None => {
            // Default: spawn server in background + TUI wizard
            let server_settings = settings.clone();
            let shutdown = CancellationToken::new();
            let shutdown_clone = shutdown.clone();
            let server_handle = tokio::spawn(async move {
                nomadflow_server::serve(server_settings, shutdown_clone, false, true)
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
