use std::process::Stdio;

use clap::{Parser, Subcommand};
use color_eyre::Result;

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
    /// Run the HTTP server only (Docker/headless mode)
    Serve,
}

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;

    let cli = Cli::parse();
    let settings = Settings::load(None).unwrap_or_default();
    settings.ensure_directories()?;

    match cli.command {
        Some(Commands::Serve) => {
            // Server-only mode with full logging
            nomadflow_server::init_tracing();
            nomadflow_server::serve(settings).await?;
        }
        None if cli.status => {
            // Status mode
            nomadflow_tui::run_status(&settings);
        }
        None if cli.attach.is_some() => {
            // Direct attach mode
            let _feature = cli.attach.unwrap();
            // Start server in background, then attach
            let server_settings = settings.clone();
            let server_handle = tokio::spawn(async move {
                nomadflow_server::serve(server_settings).await.ok();
            });

            // Give server a moment to start
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;

            // Use the TUI to switch feature, then attach
            // For direct attach, we need the server running
            let session = settings.tmux.session.clone();
            server_handle.abort();
            nomadflow_tui::exec_tmux_attach(&session);
        }
        None => {
            // Default: spawn server in background + TUI wizard
            let server_settings = settings.clone();
            let server_handle = tokio::spawn(async move {
                nomadflow_server::serve(server_settings).await.ok();
            });

            // Run TUI
            let attach_session = nomadflow_tui::run_tui(settings).await?;

            // Stop server
            server_handle.abort();

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
