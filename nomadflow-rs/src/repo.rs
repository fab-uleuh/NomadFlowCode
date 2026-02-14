use std::path::Path;
use std::process::Stdio;

use color_eyre::{eyre::eyre, Result};
use nomadflow_core::config::Settings;

pub(crate) fn link_repo(settings: &Settings, path: &Path, name: Option<&str>) -> Result<()> {
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

pub(crate) fn unlink_repo(settings: &Settings, name: Option<&str>) -> Result<()> {
    let repos_dir = settings.repos_dir();

    // Collect all symlinks in repos_dir
    let mut links: Vec<(String, std::path::PathBuf)> = Vec::new();
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
