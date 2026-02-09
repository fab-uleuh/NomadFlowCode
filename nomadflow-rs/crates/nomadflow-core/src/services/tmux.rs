use crate::error::{NomadError, Result};
use crate::shell::{command_exists, run};

pub struct TmuxService {
    session_name: String,
}

impl TmuxService {
    pub fn new(session_name: &str) -> Self {
        Self {
            session_name: session_name.to_string(),
        }
    }

    /// Ensure the tmux session exists, create if not.
    pub async fn ensure_session(&self) -> Result<bool> {
        if !command_exists("tmux").await {
            return Err(NomadError::NotFound(
                "tmux is not installed or not in PATH".to_string(),
            ));
        }

        let result = run(
            &format!("tmux has-session -t \"{}\" 2>/dev/null", self.session_name),
            None,
        )
        .await;

        if !result.success() {
            let result = run(
                &format!("tmux new-session -d -s \"{}\"", self.session_name),
                None,
            )
            .await;
            if !result.success() {
                return Err(NomadError::CommandFailed(format!(
                    "Failed to create tmux session: {}",
                    result.stderr
                )));
            }
        }

        Ok(true)
    }

    /// List all windows in the session.
    pub async fn list_windows(&self) -> Vec<TmuxWindow> {
        let result = run(
            &format!(
                "tmux list-windows -t \"{}\" -F \"#{{window_index}}:#{{window_name}}\"",
                self.session_name
            ),
            None,
        )
        .await;

        let mut windows = Vec::new();
        if result.success() {
            for line in result.stdout.trim().lines() {
                if let Some((index_str, name)) = line.split_once(':') {
                    if let Ok(index) = index_str.parse::<u32>() {
                        windows.push(TmuxWindow {
                            index,
                            name: name.to_string(),
                        });
                    }
                }
            }
        }
        windows
    }

    /// Create a new window in the session.
    pub async fn create_window(&self, name: &str, working_dir: Option<&str>) -> Result<()> {
        let mut cmd = format!(
            "tmux new-window -t \"{}\" -n \"{}\"",
            self.session_name, name
        );
        if let Some(dir) = working_dir {
            cmd.push_str(&format!(" -c \"{dir}\""));
        }

        let result = run(&cmd, None).await;
        if !result.success() {
            return Err(NomadError::CommandFailed(format!(
                "Failed to create tmux window: {}",
                result.stderr
            )));
        }
        Ok(())
    }

    /// Select/focus a window by name.
    pub async fn select_window(&self, name: &str) -> bool {
        run(
            &format!("tmux select-window -t \"{}:{}\"", self.session_name, name),
            None,
        )
        .await
        .success()
    }

    /// Kill a window by name.
    pub async fn kill_window(&self, name: &str) -> bool {
        run(
            &format!("tmux kill-window -t \"{}:{}\"", self.session_name, name),
            None,
        )
        .await
        .success()
    }

    /// Send keys to a window.
    pub async fn send_keys(&self, window: &str, keys: &str, enter: bool) -> bool {
        let enter_arg = if enter { " Enter" } else { "" };
        run(
            &format!(
                "tmux send-keys -t \"{}:{}\" \"{}\"{}",
                self.session_name, window, keys, enter_arg
            ),
            None,
        )
        .await
        .success()
    }

    /// Check if a window exists.
    pub async fn window_exists(&self, name: &str) -> bool {
        self.list_windows().await.iter().any(|w| w.name == name)
    }

    /// Get the current command running in the window's active pane.
    pub async fn get_pane_command(&self, window: &str) -> Option<String> {
        let result = run(
            &format!(
                "tmux list-panes -t \"{}:{}\" -F \"#{{pane_current_command}}\"",
                self.session_name, window
            ),
            None,
        )
        .await;
        if result.success() {
            let cmd = result.stdout.trim();
            if !cmd.is_empty() {
                return Some(cmd.lines().next().unwrap_or("").to_string());
            }
        }
        None
    }

    /// Check if the window has an idle shell.
    pub async fn is_shell_idle(&self, window: &str) -> bool {
        match self.get_pane_command(window).await {
            None => true,
            Some(cmd) => {
                const IDLE_SHELLS: &[&str] =
                    &["bash", "zsh", "sh", "fish", "dash", "ksh", "tcsh", "csh"];
                IDLE_SHELLS.contains(&cmd.to_lowercase().as_str())
            }
        }
    }

    /// Ensure a window exists, create if not.
    pub async fn ensure_window(&self, name: &str, working_dir: Option<&str>) -> Result<()> {
        if !self.window_exists(name).await {
            self.create_window(name, working_dir).await?;
            if let Some(dir) = working_dir {
                self.send_keys(name, &format!("cd \"{dir}\""), true).await;
            }
        }
        Ok(())
    }

    /// Switch to a window and optionally cd into a directory.
    /// Returns (switched_successfully, has_running_process).
    pub async fn switch_to_window(
        &self,
        name: &str,
        working_dir: Option<&str>,
    ) -> Result<(bool, bool)> {
        let window_existed = self.window_exists(name).await;
        let mut has_running_process = false;

        if window_existed {
            has_running_process = !self.is_shell_idle(name).await;
        }

        self.ensure_window(name, working_dir).await?;

        let selected = self.select_window(name).await;
        if !selected {
            return Ok((false, has_running_process));
        }

        // Only CD and clear if shell is idle
        if let Some(dir) = working_dir {
            if !has_running_process {
                self.send_keys(name, &format!("cd \"{dir}\""), true).await;
                self.send_keys(name, "clear", true).await;
            }
        }

        Ok((true, has_running_process))
    }

    pub fn session_name(&self) -> &str {
        &self.session_name
    }
}

#[derive(Debug, Clone)]
pub struct TmuxWindow {
    pub index: u32,
    pub name: String,
}

/// Build a tmux window name from repo path and feature name.
pub fn window_name(repo_path: &str, feature_name: &str) -> String {
    let repo_name = std::path::Path::new(repo_path)
        .file_name()
        .unwrap_or_default()
        .to_string_lossy();
    format!("{repo_name}:{feature_name}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmux_available() -> bool {
        std::process::Command::new("which")
            .arg("tmux")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    #[test]
    fn test_window_name() {
        assert_eq!(window_name("/home/user/repos/my-project", "add-login"), "my-project:add-login");
    }

    #[tokio::test]
    async fn test_tmux_session_lifecycle() {
        if !tmux_available() {
            eprintln!("Skipping tmux test: tmux not available");
            return;
        }

        // Use a unique session name to avoid conflicts
        let session = &format!(
            "nf-test-{}",
            std::process::id()
        );

        // Clean up any leftover session
        run(&format!("tmux kill-session -t \"{session}\" 2>/dev/null"), None).await;

        let svc = TmuxService::new(session);

        // Create session
        svc.ensure_session().await.unwrap();

        // List windows
        let windows = svc.list_windows().await;
        assert!(!windows.is_empty());

        // Create a window with a unique name
        let win = "test-lifecycle-win";
        svc.create_window(win, None).await.unwrap();
        assert!(svc.window_exists(win).await);

        // Give the shell a moment to start
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;

        // Check idle
        let idle = svc.is_shell_idle(win).await;
        assert!(idle);

        // Cleanup: kill the entire test session (more reliable than kill_window)
        run(&format!("tmux kill-session -t \"{session}\""), None).await;
    }
}
