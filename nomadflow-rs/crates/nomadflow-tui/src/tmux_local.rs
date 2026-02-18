use std::collections::BTreeMap;
use std::process::Command;

use nomadflow_core::services::tmux::parse_session_window as core_parse_session_window;

/// A tmux window entry.
#[derive(Debug, Clone)]
pub struct LocalTmuxWindow {
    pub index: u32,
    pub name: String,
    pub active: bool,
}

/// Information about a managed session window.
/// Parsed from the naming convention: `{repo}:{worktree}:{agent_type}-{n}`
#[derive(Debug, Clone)]
pub struct SessionInfo {
    pub repo: String,
    pub worktree: String,
    pub agent_type: String,
    pub agent_number: u32,
    pub window_name: String,
    pub active: bool,
    pub command: Option<String>,
}

fn exec(cmd: &str) -> Option<String> {
    let output = Command::new("sh").arg("-c").arg(cmd).output().ok()?;

    if output.status.success() {
        Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        None
    }
}

pub fn is_tmux_installed() -> bool {
    exec("which tmux").is_some()
}

pub fn session_exists(session: &str) -> bool {
    exec(&format!("tmux has-session -t \"{session}\" 2>/dev/null")).is_some()
}

pub fn list_windows(session: &str) -> Vec<LocalTmuxWindow> {
    // Use tab delimiter to avoid conflicts with colons in window names
    // (session windows use format: repo:worktree:agent-N)
    let output = match exec(&format!(
        "tmux list-windows -t \"{session}\" -F \"#{{window_index}}\t#{{window_name}}\t#{{window_active}}\""
    )) {
        Some(o) => o,
        None => return Vec::new(),
    };

    output
        .lines()
        .filter(|l| !l.is_empty())
        .filter_map(|line| {
            let parts: Vec<&str> = line.splitn(3, '\t').collect();
            if parts.len() >= 3 {
                Some(LocalTmuxWindow {
                    index: parts[0].parse().unwrap_or(0),
                    name: parts[1].to_string(),
                    active: parts[2] == "1",
                })
            } else {
                None
            }
        })
        .collect()
}

pub fn get_pane_command(session: &str, window: &str) -> Option<String> {
    exec(&format!(
        "tmux list-panes -t \"{session}:{window}\" -F \"#{{pane_current_command}}\""
    ))
}

pub fn is_shell_idle(session: &str, window: &str) -> bool {
    is_shell_idle_str(get_pane_command(session, window).as_deref())
}

pub fn is_shell_idle_str(command: Option<&str>) -> bool {
    match command {
        None => true,
        Some(cmd) => {
            let first = cmd.lines().next().unwrap_or("");
            matches!(
                first.to_lowercase().as_str(),
                "bash" | "zsh" | "sh" | "fish" | "dash" | "ksh" | "tcsh" | "csh"
            )
        }
    }
}

/// Kill a window in the given tmux session.
/// Returns `true` if the window was killed successfully.
pub fn kill_window(session: &str, window: &str) -> bool {
    let target = format!("{session}:{window}");
    Command::new("tmux")
        .args(["kill-window", "-t", &target])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Check if a window exists in the given tmux session.
pub fn window_exists(session: &str, window: &str) -> bool {
    list_windows(session).iter().any(|w| w.name == window)
}

pub fn attach_session(session: &str) {
    attach_session_target(session, None);
}

pub fn attach_session_target(session: &str, window: Option<&str>) {
    let target = match window {
        Some(w) => format!("{session}:{w}"),
        None => session.to_string(),
    };
    let _ = Command::new("tmux")
        .args(["attach-session", "-t", &target])
        .stdin(std::process::Stdio::inherit())
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .status();
}

/// Parse a window name into SessionInfo.
/// Delegates to the core `parse_session_window` and converts `Session` → `SessionInfo`.
/// Returns None for single-part names (e.g. `zsh`, `bash`).
pub fn parse_session_window(name: &str) -> Option<SessionInfo> {
    let session = core_parse_session_window(name)?;
    Some(SessionInfo {
        repo: session.repo,
        worktree: session.worktree,
        agent_type: session.agent_type,
        agent_number: session.agent_number,
        window_name: session.window_name,
        active: false,
        command: None,
    })
}

/// List all managed session windows from the tmux session.
/// Enriches each session with its active state and running command.
pub fn list_sessions(session: &str) -> Vec<SessionInfo> {
    let windows = list_windows(session);
    windows
        .iter()
        .filter_map(|w| {
            let mut info = parse_session_window(&w.name)?;
            info.active = w.active;
            info.command = get_pane_command(session, &w.name);
            Some(info)
        })
        .collect()
}

/// Group sessions by (repo, worktree), sorted alphabetically.
pub fn grouped_sessions(sessions: &[SessionInfo]) -> BTreeMap<(String, String), Vec<SessionInfo>> {
    let mut map: BTreeMap<(String, String), Vec<SessionInfo>> = BTreeMap::new();
    for s in sessions {
        map.entry((s.repo.clone(), s.worktree.clone()))
            .or_default()
            .push(s.clone());
    }
    map
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── parse_session_window tests ──

    #[test]
    fn test_parse_valid_session_window() {
        let info = parse_session_window("myapp:feature-auth:agent-1").unwrap();
        assert_eq!(info.repo, "myapp");
        assert_eq!(info.worktree, "feature-auth");
        assert_eq!(info.agent_type, "agent");
        assert_eq!(info.agent_number, 1);
        assert_eq!(info.window_name, "myapp:feature-auth:agent-1");
    }

    #[test]
    fn test_parse_legacy_2_part_returns_session() {
        let info = parse_session_window("myapp:feature-auth").unwrap();
        assert_eq!(info.repo, "myapp");
        assert_eq!(info.worktree, "feature-auth");
        assert_eq!(info.agent_type, "agent");
        assert_eq!(info.agent_number, 0);
    }

    #[test]
    fn test_parse_single_part_returns_none() {
        assert!(parse_session_window("bash").is_none());
    }

    #[test]
    fn test_parse_dashes_in_repo_and_worktree() {
        let info = parse_session_window("my-app:fix-auth-bug:agent-1").unwrap();
        assert_eq!(info.repo, "my-app");
        assert_eq!(info.worktree, "fix-auth-bug");
        assert_eq!(info.agent_type, "agent");
        assert_eq!(info.agent_number, 1);
    }

    #[test]
    fn test_parse_higher_agent_number() {
        let info = parse_session_window("myapp:feat:agent-42").unwrap();
        assert_eq!(info.agent_number, 42);
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
    fn test_parse_empty_repo_returns_none() {
        assert!(parse_session_window(":feat:agent-1").is_none());
    }

    #[test]
    fn test_parse_empty_worktree_returns_none() {
        assert!(parse_session_window("myapp::agent-1").is_none());
    }

    #[test]
    fn test_parse_empty_agent_type_returns_none() {
        // "-1" → rfind('-') at 0, agent_type is empty
        assert!(parse_session_window("myapp:feat:-1").is_none());
    }

    // ── grouped_sessions tests ──

    #[test]
    fn test_grouped_sessions_multiple_repos() {
        let sessions = vec![
            SessionInfo {
                repo: "myapp".into(),
                worktree: "feat-auth".into(),
                agent_type: "agent".into(),
                agent_number: 1,
                window_name: "myapp:feat-auth:agent-1".into(),
                active: false,
                command: None,
            },
            SessionInfo {
                repo: "myapp".into(),
                worktree: "feat-auth".into(),
                agent_type: "agent".into(),
                agent_number: 2,
                window_name: "myapp:feat-auth:agent-2".into(),
                active: false,
                command: None,
            },
            SessionInfo {
                repo: "myapp".into(),
                worktree: "fix-tests".into(),
                agent_type: "agent".into(),
                agent_number: 1,
                window_name: "myapp:fix-tests:agent-1".into(),
                active: false,
                command: None,
            },
        ];

        let grouped = grouped_sessions(&sessions);
        assert_eq!(grouped.len(), 2);
        assert_eq!(grouped[&("myapp".into(), "feat-auth".into())].len(), 2);
        assert_eq!(grouped[&("myapp".into(), "fix-tests".into())].len(), 1);
    }

    #[test]
    fn test_grouped_sessions_empty() {
        let grouped = grouped_sessions(&[]);
        assert!(grouped.is_empty());
    }

    #[test]
    fn test_grouped_sessions_sorted_by_key() {
        let sessions = vec![
            SessionInfo {
                repo: "zoo".into(),
                worktree: "b".into(),
                agent_type: "agent".into(),
                agent_number: 1,
                window_name: "zoo:b:agent-1".into(),
                active: false,
                command: None,
            },
            SessionInfo {
                repo: "alpha".into(),
                worktree: "a".into(),
                agent_type: "agent".into(),
                agent_number: 1,
                window_name: "alpha:a:agent-1".into(),
                active: false,
                command: None,
            },
        ];

        let grouped = grouped_sessions(&sessions);
        let keys: Vec<_> = grouped.keys().collect();
        // BTreeMap is sorted, so "alpha" comes before "zoo"
        assert_eq!(keys[0], &("alpha".into(), "a".into()));
        assert_eq!(keys[1], &("zoo".into(), "b".into()));
    }

    // ── list_sessions integration test (requires tmux) ──

    // ── kill_window tests ──

    #[test]
    fn test_kill_window_removes_window() {
        if !is_tmux_installed() {
            eprintln!("Skipping: tmux not installed");
            return;
        }

        let pid = std::process::id();
        let session_name = format!("nf-test-kill-{pid}");
        let window_name = "myapp:feat:agent-1";

        // Create session with a window
        let created = exec(&format!(
            "tmux new-session -d -s \"{session_name}\" -n \"{window_name}\""
        ));
        if created.is_none() {
            eprintln!("Skipping: could not create tmux session");
            return;
        }

        // Add a second window so killing the first doesn't destroy the session
        exec(&format!(
            "tmux new-window -t \"{session_name}\" -n \"myapp:feat:agent-2\""
        ));

        assert!(window_exists(&session_name, window_name));

        let killed = kill_window(&session_name, window_name);
        assert!(killed);
        assert!(!window_exists(&session_name, window_name));

        // Cleanup
        exec(&format!("tmux kill-session -t \"{session_name}\""));
    }

    #[test]
    fn test_kill_window_nonexistent_returns_false() {
        if !is_tmux_installed() {
            eprintln!("Skipping: tmux not installed");
            return;
        }

        let pid = std::process::id();
        let session_name = format!("nf-test-killnx-{pid}");

        // Create session with one window
        let created = exec(&format!(
            "tmux new-session -d -s \"{session_name}\" -n \"myapp:feat:agent-1\""
        ));
        if created.is_none() {
            eprintln!("Skipping: could not create tmux session");
            return;
        }

        let killed = kill_window(&session_name, "nonexistent:window:agent-99");
        assert!(!killed);

        // Cleanup
        exec(&format!("tmux kill-session -t \"{session_name}\""));
    }

    #[test]
    fn test_list_sessions_with_real_tmux() {
        if !is_tmux_installed() {
            eprintln!("Skipping: tmux not installed");
            return;
        }

        let pid = std::process::id();
        let session_name = format!("nf-test-sessions-{pid}");

        // Create a tmux session with session-format windows
        let created = exec(&format!(
            "tmux new-session -d -s \"{session_name}\" -n \"myapp:feat:agent-1\""
        ));
        if created.is_none() {
            eprintln!("Skipping: could not create tmux session");
            return;
        }

        // Add another session window
        exec(&format!(
            "tmux new-window -t \"{session_name}\" -n \"myapp:feat:agent-2\""
        ));
        // Add a legacy 2-part window (now included in list_sessions)
        exec(&format!(
            "tmux new-window -t \"{session_name}\" -n \"myapp:feat\""
        ));

        let sessions = list_sessions(&session_name);

        // Cleanup
        exec(&format!("tmux kill-session -t \"{session_name}\""));

        // Should find 3 session windows: 2 new-format + 1 legacy
        assert_eq!(sessions.len(), 3);
        assert_eq!(sessions[0].repo, "myapp");
        assert_eq!(sessions[0].worktree, "feat");
        assert_eq!(sessions[0].agent_type, "agent");
        assert_eq!(sessions[0].agent_number, 1);
        assert_eq!(sessions[1].agent_number, 2);
        // Legacy window
        assert_eq!(sessions[2].repo, "myapp");
        assert_eq!(sessions[2].worktree, "feat");
        assert_eq!(sessions[2].agent_type, "agent");
        assert_eq!(sessions[2].agent_number, 0);
    }
}
