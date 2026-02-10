use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;

use clap::{Parser, Subcommand};
use color_eyre::Result;
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

    /// Attach directly to a feature
    #[arg(long)]
    attach: Option<String>,
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
        None if cli.status => {
            show_daemon_status(&settings);
            nomadflow_tui::run_status(&settings);
        }
        None if cli.attach.is_some() => {
            let _feature = cli.attach.unwrap();
            let server_settings = settings.clone();
            let shutdown = CancellationToken::new();
            let shutdown_clone = shutdown.clone();
            let server_handle = tokio::spawn(async move {
                nomadflow_server::serve(server_settings, shutdown_clone, false, true)
                    .await
                    .ok();
            });

            // Give server a moment to start
            tokio::time::sleep(Duration::from_millis(200)).await;

            let session = settings.tmux.session.clone();

            // Graceful shutdown instead of abort
            shutdown.cancel();
            tokio::select! {
                _ = server_handle => {}
                _ = tokio::time::sleep(Duration::from_secs(5)) => {}
            }

            nomadflow_tui::exec_tmux_attach(&session);
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
