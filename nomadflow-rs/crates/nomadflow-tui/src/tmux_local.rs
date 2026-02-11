use std::process::Command;

/// A tmux window entry.
#[derive(Debug, Clone)]
pub struct LocalTmuxWindow {
    pub index: u32,
    pub name: String,
    pub active: bool,
}

fn exec(cmd: &str) -> Option<String> {
    let output = Command::new("sh")
        .arg("-c")
        .arg(cmd)
        .output()
        .ok()?;

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
    let output = match exec(&format!(
        "tmux list-windows -t \"{session}\" -F \"#{{window_index}}:#{{window_name}}:#{{window_active}}\""
    )) {
        Some(o) => o,
        None => return Vec::new(),
    };

    output
        .lines()
        .filter(|l| !l.is_empty())
        .filter_map(|line| {
            let parts: Vec<&str> = line.splitn(3, ':').collect();
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
