use std::time::Duration;

use tokio::process::Command;

#[derive(Debug)]
pub struct CommandResult {
    pub stdout: String,
    pub stderr: String,
    pub return_code: i32,
}

impl CommandResult {
    pub fn success(&self) -> bool {
        self.return_code == 0
    }
}

/// Run a shell command asynchronously with a timeout.
pub async fn run_command(
    command: &str,
    cwd: Option<&str>,
    timeout_secs: f64,
) -> CommandResult {
    let mut cmd = Command::new("sh");
    cmd.arg("-c").arg(command);

    if let Some(dir) = cwd {
        cmd.current_dir(dir);
    }

    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());

    let result = tokio::time::timeout(
        Duration::from_secs_f64(timeout_secs),
        async {
            let child = cmd.spawn();
            match child {
                Ok(child) => child.wait_with_output().await,
                Err(e) => Err(e),
            }
        },
    )
    .await;

    match result {
        Ok(Ok(output)) => CommandResult {
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            return_code: output.status.code().unwrap_or(-1),
        },
        Ok(Err(e)) => CommandResult {
            stdout: String::new(),
            stderr: format!("Failed to execute command: {e}"),
            return_code: -1,
        },
        Err(_) => CommandResult {
            stdout: String::new(),
            stderr: format!("Command timed out after {timeout_secs}s"),
            return_code: -1,
        },
    }
}

/// Run a shell command with the default 30s timeout.
pub async fn run(command: &str, cwd: Option<&str>) -> CommandResult {
    run_command(command, cwd, 30.0).await
}

/// Check if a command exists in PATH.
pub async fn command_exists(name: &str) -> bool {
    run(&format!("which {name}"), None).await.success()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_run_echo() {
        let result = run("echo hello", None).await;
        assert!(result.success());
        assert_eq!(result.stdout.trim(), "hello");
    }

    #[tokio::test]
    async fn test_run_failure() {
        let result = run("false", None).await;
        assert!(!result.success());
    }

    #[tokio::test]
    async fn test_timeout() {
        let result = run_command("sleep 10", None, 0.1).await;
        assert_eq!(result.return_code, -1);
        assert!(result.stderr.contains("timed out"));
    }

    #[tokio::test]
    async fn test_command_exists_git() {
        assert!(command_exists("git").await);
    }

    #[tokio::test]
    async fn test_command_not_exists() {
        assert!(!command_exists("nonexistent_xyz_12345").await);
    }
}
