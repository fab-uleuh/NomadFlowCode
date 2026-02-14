use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;

use color_eyre::Result;
use nomadflow_core::config::Settings;

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

pub(crate) fn start_daemon(settings: &Settings) -> Result<()> {
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

pub(crate) fn stop_daemon(settings: &Settings) -> Result<()> {
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

pub(crate) fn show_status(settings: &Settings) {
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
