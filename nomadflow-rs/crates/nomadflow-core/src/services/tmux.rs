use crate::error::{NomadError, Result};
use crate::models::Session;
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

            // Hide tmux status bar on newly created sessions — NomadFlow manages
            // sessions at app level, the green bar wastes screen space on mobile.
            let _ = run(
                &format!(
                    "tmux set-option -t \"{}\" status off",
                    self.session_name
                ),
                None,
            )
            .await;
        }

        Ok(true)
    }

    /// List all windows in the session.
    pub async fn list_windows(&self) -> Vec<TmuxWindow> {
        let result = run(
            // Use tab delimiter to avoid conflicts with colons in window names
            // (session windows use format: repo:worktree:agent-N)
            &format!(
                "tmux list-windows -t \"{}\" -F \"#{{window_index}}\t#{{window_name}}\"",
                self.session_name
            ),
            None,
        )
        .await;

        let mut windows = Vec::new();
        if result.success() {
            for line in result.stdout.trim().lines() {
                if let Some((index_str, name)) = line.split_once('\t') {
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
    /// If `session_override` is provided, targets that specific linked session
    /// instead of the base session (for independent cursor per client).
    pub async fn select_window(&self, name: &str) -> bool {
        self.select_window_in(name, None).await
    }

    /// Select/focus a window in a specific session (or base session if None).
    pub async fn select_window_in(&self, name: &str, session_override: Option<&str>) -> bool {
        let target_session = session_override.unwrap_or(&self.session_name);
        run(
            &format!("tmux select-window -t \"{}:{}\"", target_session, name),
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

    /// Get the current working directory of the window's active pane.
    pub async fn get_pane_cwd(&self, window: &str) -> Option<String> {
        let result = run(
            &format!(
                "tmux display-message -t \"{}:{}\" -p \"#{{pane_current_path}}\"",
                self.session_name, window
            ),
            None,
        )
        .await;
        if result.success() {
            let cwd = result.stdout.trim();
            if !cwd.is_empty() {
                return Some(cwd.to_string());
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
    /// When `session_override` is provided, select the window in that linked session
    /// instead of the base session (for independent cursor support).
    /// Returns (switched_successfully, has_running_process).
    pub async fn switch_to_window(
        &self,
        name: &str,
        working_dir: Option<&str>,
        session_override: Option<&str>,
    ) -> Result<(bool, bool)> {
        let window_existed = self.window_exists(name).await;
        let mut has_running_process = false;

        if window_existed {
            has_running_process = !self.is_shell_idle(name).await;
        }

        self.ensure_window(name, working_dir).await?;

        let selected = self.select_window_in(name, session_override).await;
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

    /// Find the next agent number for a given repo:worktree prefix.
    ///
    /// Scans existing windows matching `{repo}:{worktree}:*`, parses `agent-N` suffixes,
    /// and returns max(N) + 1. Returns 1 if no matching windows exist.
    pub async fn next_agent_number(&self, repo: &str, worktree: &str) -> Result<u32> {
        let prefix = format!("{repo}:{worktree}:");
        let windows = self.list_windows().await;

        let max_num = windows
            .iter()
            .filter(|w| w.name.starts_with(&prefix))
            .filter_map(|w| {
                let suffix = &w.name[prefix.len()..];
                // Parse "agent-N" or "{type}-N"
                suffix.rsplit('-').next()?.parse::<u32>().ok()
            })
            .max();

        match max_num {
            Some(n) if n < 99 => Ok(n + 1),
            Some(_) => Ok(99), // cap at 99
            None => Ok(1),
        }
    }

    /// List all managed session windows.
    ///
    /// Calls `list_windows()`, filters with `parse_session_window()`,
    /// and returns valid session windows (both 3-part and legacy 2-part).
    pub async fn list_sessions(&self) -> Vec<Session> {
        self.list_windows()
            .await
            .iter()
            .filter_map(|w| parse_session_window(&w.name))
            .collect()
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

/// Sanitize a string for use in tmux window names.
///
/// Replaces spaces, slashes, dots, and other special chars with dashes.
/// Dots are specifically excluded because tmux uses `.` as the pane separator
/// in target syntax (`session:window.pane`).
fn sanitize_for_tmux(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '_' || c == '-' {
                c
            } else {
                '-'
            }
        })
        .collect()
}

/// Build a session window name for agent sessions: `{repo}:{worktree}:{agent_type}-{n}`.
///
/// Sanitizes all components: replaces spaces, slashes, and dots with dashes (tmux limitation).
pub fn session_window_name(repo: &str, worktree: &str, agent_type: &str, number: u32) -> String {
    format!(
        "{}:{}:{}-{}",
        sanitize_for_tmux(repo),
        sanitize_for_tmux(worktree),
        sanitize_for_tmux(agent_type),
        number
    )
}

/// Parse a window name into a Session.
///
/// Supports two formats:
/// - 3-part: `{repo}:{worktree}:{agent_type}-{n}` (new session windows)
/// - 2-part: `{repo}:{worktree}` (legacy windows, mapped to agent_type="agent", agent_number=0)
///
/// Returns None for single-part names (e.g. `zsh`, `bash`).
pub fn parse_session_window(name: &str) -> Option<Session> {
    let parts: Vec<&str> = name.splitn(3, ':').collect();

    match parts.len() {
        3 => {
            let repo = parts[0];
            let worktree = parts[1];
            let agent_part = parts[2];

            // Split agent_part on last '-' to get agent_type and number
            let dash_pos = agent_part.rfind('-')?;
            let agent_type = &agent_part[..dash_pos];
            let number_str = &agent_part[dash_pos + 1..];
            let agent_number: u32 = number_str.parse().ok()?;

            if agent_type.is_empty() || repo.is_empty() || worktree.is_empty() {
                return None;
            }

            let session_id = name.replace(':', "-");

            Some(Session {
                session_id,
                window_name: name.to_string(),
                repo: repo.to_string(),
                worktree: worktree.to_string(),
                agent_type: agent_type.to_string(),
                agent_number,
            })
        }
        2 => {
            let repo = parts[0];
            let worktree = parts[1];

            if repo.is_empty() || worktree.is_empty() {
                return None;
            }

            let session_id = name.replace(':', "-");

            Some(Session {
                session_id,
                window_name: name.to_string(),
                repo: repo.to_string(),
                worktree: worktree.to_string(),
                agent_type: "agent".to_string(),
                agent_number: 0,
            })
        }
        _ => None,
    }
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
        assert_eq!(
            window_name("/home/user/repos/my-project", "add-login"),
            "my-project:add-login"
        );
    }

    #[test]
    fn test_session_window_name() {
        assert_eq!(
            session_window_name("myapp", "feature-auth", "agent", 1),
            "myapp:feature-auth:agent-1"
        );
        assert_eq!(
            session_window_name("myapp", "fix-tests", "agent", 2),
            "myapp:fix-tests:agent-2"
        );
        // Dots should be sanitized (tmux uses . as pane separator)
        assert_eq!(
            session_window_name("myapp", "v2.0", "agent", 1),
            "myapp:v2-0:agent-1"
        );
        // Slashes and spaces should be sanitized
        assert_eq!(
            session_window_name("my app", "feat/login", "agent", 1),
            "my-app:feat-login:agent-1"
        );
    }

    #[tokio::test]
    async fn test_next_agent_number() {
        if !tmux_available() {
            eprintln!("Skipping tmux test: tmux not available");
            return;
        }

        let session = &format!("nf-test-agent-{}", std::process::id());
        // Clean up any leftover session
        run(
            &format!("tmux kill-session -t \"{session}\" 2>/dev/null"),
            None,
        )
        .await;

        let svc = TmuxService::new(session);
        svc.ensure_session().await.unwrap();

        // No matching windows → should return 1
        let n = svc.next_agent_number("testrepo", "feat").await.unwrap();
        assert_eq!(n, 1);

        // Create a window with agent-1 naming
        svc.create_window("testrepo:feat:agent-1", None)
            .await
            .unwrap();
        let n = svc.next_agent_number("testrepo", "feat").await.unwrap();
        assert_eq!(n, 2);

        // Create agent-3 (gap in numbering) → should return 4
        svc.create_window("testrepo:feat:agent-3", None)
            .await
            .unwrap();
        let n = svc.next_agent_number("testrepo", "feat").await.unwrap();
        assert_eq!(n, 4);

        // Cleanup
        run(&format!("tmux kill-session -t \"{session}\""), None).await;
    }

    #[tokio::test]
    async fn test_full_session_flow() {
        if !tmux_available() {
            eprintln!("Skipping tmux test: tmux not available");
            return;
        }

        let session = &format!("nf-test-flow-{}", std::process::id());
        crate::shell::run(
            &format!("tmux kill-session -t \"{session}\" 2>/dev/null"),
            None,
        )
        .await;

        let svc = TmuxService::new(session);
        svc.ensure_session().await.unwrap();

        // Simulate creating agent windows for a repo:worktree
        let repo = "myapp";
        let worktree = "feat-auth";

        // First agent
        let n1 = svc.next_agent_number(repo, worktree).await.unwrap();
        assert_eq!(n1, 1);
        let win1 = session_window_name(repo, worktree, "agent", n1);
        assert_eq!(win1, "myapp:feat-auth:agent-1");
        svc.create_window(&win1, None).await.unwrap();

        // Second agent (AC #2 — increment)
        let n2 = svc.next_agent_number(repo, worktree).await.unwrap();
        assert_eq!(n2, 2);
        let win2 = session_window_name(repo, worktree, "agent", n2);
        assert_eq!(win2, "myapp:feat-auth:agent-2");
        svc.create_window(&win2, None).await.unwrap();

        // Both windows should exist
        assert!(svc.window_exists(&win1).await);
        assert!(svc.window_exists(&win2).await);

        // Different worktree should start at 1
        let n_other = svc.next_agent_number(repo, "other-feat").await.unwrap();
        assert_eq!(n_other, 1);

        // Cleanup
        crate::shell::run(&format!("tmux kill-session -t \"{session}\""), None).await;
    }

    #[tokio::test]
    async fn test_tmux_session_lifecycle() {
        if !tmux_available() {
            eprintln!("Skipping tmux test: tmux not available");
            return;
        }

        // Use a unique session name to avoid conflicts
        let session = &format!("nf-test-{}", std::process::id());

        // Clean up any leftover session
        run(
            &format!("tmux kill-session -t \"{session}\" 2>/dev/null"),
            None,
        )
        .await;

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

    // ── parse_session_window tests ──

    #[test]
    fn test_parse_valid_session_window() {
        let s = parse_session_window("myapp:feature-auth:agent-1").unwrap();
        assert_eq!(s.repo, "myapp");
        assert_eq!(s.worktree, "feature-auth");
        assert_eq!(s.agent_type, "agent");
        assert_eq!(s.agent_number, 1);
        assert_eq!(s.window_name, "myapp:feature-auth:agent-1");
        assert_eq!(s.session_id, "myapp-feature-auth-agent-1");
    }

    #[test]
    fn test_parse_legacy_2_part_returns_session() {
        let s = parse_session_window("myapp:feature-auth").unwrap();
        assert_eq!(s.repo, "myapp");
        assert_eq!(s.worktree, "feature-auth");
        assert_eq!(s.agent_type, "agent");
        assert_eq!(s.agent_number, 0);
        assert_eq!(s.window_name, "myapp:feature-auth");
    }

    #[test]
    fn test_parse_legacy_2_part_session_id() {
        let s = parse_session_window("myapp:feature-auth").unwrap();
        assert_eq!(s.session_id, "myapp-feature-auth");
    }

    #[test]
    fn test_parse_legacy_2_part_empty_parts_return_none() {
        assert!(parse_session_window(":feature").is_none());
        assert!(parse_session_window("myapp:").is_none());
        assert!(parse_session_window(":").is_none());
    }

    #[test]
    fn test_parse_single_part_returns_none() {
        assert!(parse_session_window("bash").is_none());
    }

    #[test]
    fn test_parse_dashes_in_repo_and_worktree() {
        let s = parse_session_window("my-app:fix-auth-bug:claude-1").unwrap();
        assert_eq!(s.repo, "my-app");
        assert_eq!(s.worktree, "fix-auth-bug");
        assert_eq!(s.agent_type, "claude");
        assert_eq!(s.agent_number, 1);
    }

    #[test]
    fn test_parse_higher_agent_number() {
        let s = parse_session_window("myapp:feat:agent-42").unwrap();
        assert_eq!(s.agent_number, 42);
    }

    #[test]
    fn test_parse_invalid_number_returns_none() {
        assert!(parse_session_window("myapp:feat:agent-abc").is_none());
    }

    #[test]
    fn test_parse_no_dash_in_agent_returns_none() {
        assert!(parse_session_window("myapp:feat:agent").is_none());
    }

    #[test]
    fn test_parse_empty_parts_return_none() {
        assert!(parse_session_window(":feat:agent-1").is_none());
        assert!(parse_session_window("myapp::agent-1").is_none());
        assert!(parse_session_window("myapp:feat:-1").is_none());
    }

    // ── list_sessions integration test ──

    #[tokio::test]
    async fn test_list_sessions_filters_correctly() {
        if !tmux_available() {
            eprintln!("Skipping tmux test: tmux not available");
            return;
        }

        let session = &format!("nf-test-ls-{}", std::process::id());
        run(
            &format!("tmux kill-session -t \"{session}\" 2>/dev/null"),
            None,
        )
        .await;

        let svc = TmuxService::new(session);
        svc.ensure_session().await.unwrap();

        // Create 2 session windows
        let win1 = session_window_name("myapp", "feat", "agent", 1);
        let win2 = session_window_name("myapp", "feat", "agent", 2);
        svc.create_window(&win1, None).await.unwrap();
        svc.create_window(&win2, None).await.unwrap();

        // Create a legacy 2-part window (now included in list_sessions)
        svc.create_window("myapp:feat-legacy", None).await.unwrap();

        let sessions = svc.list_sessions().await;

        // Cleanup
        run(&format!("tmux kill-session -t \"{session}\""), None).await;

        // Should find 3 session windows: 2 new-format + 1 legacy
        assert_eq!(sessions.len(), 3);
        assert_eq!(sessions[0].repo, "myapp");
        assert_eq!(sessions[0].worktree, "feat");
        assert_eq!(sessions[0].agent_type, "agent");
        assert_eq!(sessions[0].agent_number, 1);
        assert_eq!(sessions[1].agent_number, 2);
        assert_eq!(sessions[0].session_id, "myapp-feat-agent-1");
        // Legacy window
        assert_eq!(sessions[2].repo, "myapp");
        assert_eq!(sessions[2].worktree, "feat-legacy");
        assert_eq!(sessions[2].agent_type, "agent");
        assert_eq!(sessions[2].agent_number, 0);
    }

    #[tokio::test]
    async fn test_select_window_in_with_linked_session() {
        if !tmux_available() {
            eprintln!("Skipping tmux test: tmux not available");
            return;
        }

        let base_session = &format!("nf-test-linked-{}", std::process::id());
        run(
            &format!("tmux kill-session -t \"{base_session}\" 2>/dev/null"),
            None,
        )
        .await;

        let svc = TmuxService::new(base_session);
        svc.ensure_session().await.unwrap();

        // Create two windows in the base session
        svc.create_window("win-a", None).await.unwrap();
        svc.create_window("win-b", None).await.unwrap();

        // Create a linked session (session group)
        let linked_result = run(
            &format!("tmux new-session -d -t \"{base_session}\""),
            None,
        )
        .await;
        assert!(linked_result.success(), "Failed to create linked session");

        // Discover the linked session name
        let list_result = run(
            &format!(
                "tmux list-sessions -F \"#{{session_name}} #{{session_group}}\" 2>/dev/null"
            ),
            None,
        )
        .await;
        let linked_name = list_result
            .stdout
            .trim()
            .lines()
            .filter_map(|line| {
                let mut parts = line.splitn(2, ' ');
                let name = parts.next()?;
                let group = parts.next().unwrap_or("");
                if group == *base_session && name != *base_session {
                    Some(name.to_string())
                } else {
                    None
                }
            })
            .next()
            .expect("Should find a linked session");

        // select_window_in with session override should succeed
        let selected = svc.select_window_in("win-a", Some(&linked_name)).await;
        assert!(selected, "select_window_in with linked session should succeed");

        // select_window_in with None (base session) should also succeed
        let selected_base = svc.select_window_in("win-b", None).await;
        assert!(selected_base, "select_window_in with base session should succeed");

        // Cleanup
        run(&format!("tmux kill-session -t \"{base_session}\""), None).await;
    }
}
