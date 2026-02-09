use std::net::TcpListener;

use tokio::process::{Child, Command};

use crate::config::Settings;
use crate::error::{NomadError, Result};
use crate::shell::command_exists;

pub struct TtydService {
    port: u16,
    session_name: String,
    secret: String,
    process: Option<Child>,
}

impl TtydService {
    pub fn new(settings: &Settings) -> Self {
        Self {
            port: settings.ttyd.port,
            session_name: settings.tmux.session.clone(),
            secret: settings.auth.secret.clone(),
            process: None,
        }
    }

    /// Start the ttyd subprocess.
    pub async fn start(&mut self) -> Result<()> {
        if !command_exists("ttyd").await {
            return Err(NomadError::NotFound(
                "ttyd is not installed or not in PATH. \
                 Install with: brew install ttyd (macOS) or apt install ttyd (Linux)"
                    .to_string(),
            ));
        }

        if self.port_in_use() {
            return Ok(());
        }

        let mut cmd = Command::new("ttyd");
        cmd.arg("-p")
            .arg(self.port.to_string())
            .arg("-W");

        if !self.secret.is_empty() {
            cmd.arg("-c")
                .arg(format!("nomadflow:{}", self.secret));
        }

        cmd.arg("tmux")
            .arg("attach-session")
            .arg("-t")
            .arg(&self.session_name);

        cmd.stdout(std::process::Stdio::null());
        cmd.stderr(std::process::Stdio::null());

        let child = cmd.spawn().map_err(|e| {
            NomadError::CommandFailed(format!("Failed to start ttyd: {e}"))
        })?;

        self.process = Some(child);

        // Give it a moment to start
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;

        Ok(())
    }

    /// Stop the ttyd subprocess.
    pub async fn stop(&mut self) {
        if let Some(ref mut child) = self.process {
            child.kill().await.ok();
            child.wait().await.ok();
        }
        self.process = None;
    }

    fn port_in_use(&self) -> bool {
        TcpListener::bind(("127.0.0.1", self.port)).is_err()
    }

    pub fn port(&self) -> u16 {
        self.port
    }
}
