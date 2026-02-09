use std::path::{Path, PathBuf};

use crate::config::Settings;
use crate::error::{NomadError, Result};
use crate::models::{Feature, Repository};
use crate::shell::{run, run_command};

pub struct GitService {
    repos_dir: PathBuf,
    worktrees_dir: PathBuf,
}

impl GitService {
    pub fn new(settings: &Settings) -> Self {
        Self {
            repos_dir: settings.repos_dir(),
            worktrees_dir: settings.worktrees_dir(),
        }
    }

    /// List all Git repositories in the repos directory.
    pub async fn list_repos(&self) -> Result<Vec<Repository>> {
        let mut repos = Vec::new();

        if !self.repos_dir.exists() {
            return Ok(repos);
        }

        let mut entries = tokio::fs::read_dir(&self.repos_dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.is_dir() && path.join(".git").exists() {
                let branch = self.get_current_branch(&path).await;
                repos.push(Repository {
                    name: path.file_name().unwrap_or_default().to_string_lossy().to_string(),
                    path: path.to_string_lossy().to_string(),
                    branch,
                });
            }
        }

        Ok(repos)
    }

    /// Clone a git repository into the repos directory.
    pub async fn clone_repo(
        &self,
        url: &str,
        token: Option<&str>,
        name: Option<&str>,
    ) -> Result<(String, String, String)> {
        // Extract repo name from URL if not provided
        let repo_name = match name {
            Some(n) if !n.is_empty() => n.to_string(),
            _ => {
                let path_part = url.trim_end_matches('/');
                let stem = Path::new(path_part)
                    .file_stem()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_default();
                if stem.is_empty() {
                    return Err(NomadError::Other(
                        "Cannot determine repository name from URL".to_string(),
                    ));
                }
                stem
            }
        };

        // Sanitize name
        let repo_name = sanitize_name(&repo_name);

        let dest = self.repos_dir.join(&repo_name);
        if dest.exists() {
            return Err(NomadError::AlreadyExists(format!(
                "Repository '{repo_name}' already exists"
            )));
        }

        // Ensure repos directory exists
        tokio::fs::create_dir_all(&self.repos_dir).await?;

        // Build clone URL with token if provided
        let clone_url = if let Some(tok) = token {
            inject_token(url, tok)
        } else {
            url.to_string()
        };

        let dest_str = dest.to_string_lossy();
        let result = run_command(
            &format!("git clone {clone_url} {dest_str}"),
            None,
            600.0,
        )
        .await;

        if !result.success() {
            return Err(NomadError::CommandFailed(format!(
                "git clone failed: {}",
                result.stderr
            )));
        }

        // Security: remove token from remote URL
        if token.is_some() {
            run(
                &format!("git remote set-url origin {url}"),
                Some(&dest_str),
            )
            .await;
        }

        let branch = self.get_current_branch(&dest).await;
        Ok((repo_name, dest.to_string_lossy().to_string(), branch))
    }

    /// List all worktrees (features) for a repository.
    pub async fn list_features(&self, repo_path: &str) -> Result<Vec<Feature>> {
        let mut features = Vec::new();
        let repo_path_obj = PathBuf::from(repo_path);
        let repo_name = repo_path_obj
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        // Canonicalize repo_path for reliable comparison with worktree paths
        let canonical_repo = std::fs::canonicalize(repo_path)
            .unwrap_or_else(|_| PathBuf::from(repo_path));

        let result = run("git worktree list --porcelain", Some(repo_path)).await;
        if !result.success() {
            return Ok(features);
        }

        // Parse worktree list output
        let mut current_worktree: Option<String> = None;
        let mut current_branch: Option<String> = None;

        for line in result.stdout.lines() {
            let line = line.trim();
            if line.is_empty() {
                if let Some(wt_path) = current_worktree.take() {
                    let branch = current_branch.take().unwrap_or_default();
                    let branch = branch
                        .strip_prefix("refs/heads/")
                        .unwrap_or(&branch)
                        .to_string();

                    let canonical_wt = std::fs::canonicalize(&wt_path)
                        .unwrap_or_else(|_| PathBuf::from(&wt_path));
                    let is_main = canonical_wt == canonical_repo;
                    let name = if is_main {
                        if branch.is_empty() {
                            repo_name.clone()
                        } else {
                            branch.clone()
                        }
                    } else {
                        Path::new(&wt_path)
                            .file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .to_string()
                    };

                    features.push(Feature {
                        name,
                        worktree_path: wt_path,
                        branch,
                        is_active: false,
                        is_main,
                    });
                }
                current_branch = None;
            } else if let Some(rest) = line.strip_prefix("worktree ") {
                current_worktree = Some(rest.to_string());
            } else if let Some(rest) = line.strip_prefix("branch ") {
                current_branch = Some(rest.to_string());
            }
        }

        // Handle last worktree if no trailing newline
        if let Some(wt_path) = current_worktree.take() {
            let branch = current_branch.take().unwrap_or_default();
            let branch = branch
                .strip_prefix("refs/heads/")
                .unwrap_or(&branch)
                .to_string();
            let canonical_wt = std::fs::canonicalize(&wt_path)
                .unwrap_or_else(|_| PathBuf::from(&wt_path));
            let is_main = canonical_wt == canonical_repo;
            let name = if is_main {
                if branch.is_empty() { repo_name.clone() } else { branch.clone() }
            } else {
                Path::new(&wt_path)
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string()
            };
            features.push(Feature {
                name,
                worktree_path: wt_path,
                branch,
                is_active: false,
                is_main,
            });
        }

        // Also check the worktrees directory for this repo
        let repo_worktrees_dir = self.worktrees_dir.join(&repo_name);
        if repo_worktrees_dir.exists() {
            let existing_paths: std::collections::HashSet<String> =
                features.iter().map(|f| f.worktree_path.clone()).collect();

            let mut entries = tokio::fs::read_dir(&repo_worktrees_dir).await?;
            while let Some(entry) = entries.next_entry().await? {
                let path = entry.path();
                if path.is_dir() && !existing_paths.contains(&path.to_string_lossy().to_string()) {
                    let branch = self.get_current_branch(&path).await;
                    features.push(Feature {
                        name: path.file_name().unwrap_or_default().to_string_lossy().to_string(),
                        worktree_path: path.to_string_lossy().to_string(),
                        branch,
                        is_active: false,
                        is_main: false,
                    });
                }
            }
        }

        Ok(features)
    }

    /// Create a new feature worktree.
    pub async fn create_feature(
        &self,
        repo_path: &str,
        feature_name: &str,
        base_branch: Option<&str>,
    ) -> Result<(String, String)> {
        let repo_path_obj = PathBuf::from(repo_path);
        let repo_name = repo_path_obj
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        // Auto-detect default branch if not specified
        let base = match base_branch {
            Some(b) if !b.is_empty() => b.to_string(),
            _ => self.get_default_branch(repo_path).await,
        };

        let repo_worktrees_dir = self.worktrees_dir.join(&repo_name);
        tokio::fs::create_dir_all(&repo_worktrees_dir).await?;

        let worktree_path = repo_worktrees_dir.join(feature_name);
        let branch_name = format!("feature/{feature_name}");

        // If worktree already exists, just return
        if worktree_path.exists() {
            return Ok((worktree_path.to_string_lossy().to_string(), branch_name));
        }

        // Fetch latest from remote (ignore errors)
        run("git fetch --all 2>/dev/null || true", Some(repo_path)).await;

        let wt = worktree_path.to_string_lossy();

        // Try to create with new branch
        let result = run(
            &format!("git worktree add -b \"{branch_name}\" \"{wt}\" \"{base}\""),
            Some(repo_path),
        )
        .await;

        if !result.success() {
            // Branch might already exist
            let result = run(
                &format!("git worktree add \"{wt}\" \"{branch_name}\""),
                Some(repo_path),
            )
            .await;

            if !result.success() {
                // Try with origin/base
                let result = run(
                    &format!("git worktree add -b \"{branch_name}\" \"{wt}\" \"origin/{base}\""),
                    Some(repo_path),
                )
                .await;

                if !result.success() {
                    // Last resort: from HEAD
                    let result = run(
                        &format!("git worktree add -b \"{branch_name}\" \"{wt}\" HEAD"),
                        Some(repo_path),
                    )
                    .await;

                    if !result.success() {
                        return Err(NomadError::CommandFailed(format!(
                            "Failed to create worktree: {}",
                            result.stderr
                        )));
                    }
                }
            }
        }

        Ok((worktree_path.to_string_lossy().to_string(), branch_name))
    }

    /// Delete a feature worktree.
    pub async fn delete_feature(
        &self,
        repo_path: &str,
        feature_name: &str,
    ) -> Result<bool> {
        let repo_path_obj = PathBuf::from(repo_path);
        let repo_name = repo_path_obj
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        let worktree_path = self.worktrees_dir.join(&repo_name).join(feature_name);
        let wt = worktree_path.to_string_lossy();

        let result = run(
            &format!("git worktree remove \"{wt}\" --force"),
            Some(repo_path),
        )
        .await;

        if !result.success() {
            run("git worktree prune", Some(repo_path)).await;

            if worktree_path.exists() {
                tokio::fs::remove_dir_all(&worktree_path).await.ok();
            }
        }

        // Delete the branch
        let branch_name = format!("feature/{feature_name}");
        run(
            &format!("git branch -D \"{branch_name}\""),
            Some(repo_path),
        )
        .await;

        Ok(true)
    }

    /// Get the current branch of a repository.
    async fn get_current_branch(&self, repo_path: &Path) -> String {
        let result = run(
            "git rev-parse --abbrev-ref HEAD",
            Some(&repo_path.to_string_lossy()),
        )
        .await;
        if result.success() {
            result.stdout.trim().to_string()
        } else {
            "unknown".to_string()
        }
    }

    /// Get the default branch of a repository.
    pub async fn get_default_branch(&self, repo_path: &str) -> String {
        // Try to get from remote HEAD
        let result = run(
            "git symbolic-ref refs/remotes/origin/HEAD 2>/dev/null | sed 's@^refs/remotes/origin/@@'",
            Some(repo_path),
        )
        .await;
        if result.success() {
            let branch = result.stdout.trim();
            if !branch.is_empty() {
                return branch.to_string();
            }
        }

        // Check common branches
        for branch in &["main", "master", "develop", "dev"] {
            let result = run(
                &format!("git rev-parse --verify \"{branch}\" 2>/dev/null"),
                Some(repo_path),
            )
            .await;
            if result.success() {
                return branch.to_string();
            }
        }

        // Fall back to current branch
        let result = run("git rev-parse --abbrev-ref HEAD", Some(repo_path)).await;
        if result.success() {
            return result.stdout.trim().to_string();
        }

        "main".to_string()
    }
}

/// Sanitize a repository name: replace non-alphanumeric chars (except ._-) with dashes.
pub fn sanitize_name(name: &str) -> String {
    let mut result = String::with_capacity(name.len());
    for c in name.chars() {
        if c.is_alphanumeric() || c == '.' || c == '_' || c == '-' {
            result.push(c);
        } else {
            result.push('-');
        }
    }
    result
}

/// Inject a token into a git HTTPS URL.
fn inject_token(url: &str, token: &str) -> String {
    if let Some(rest) = url.strip_prefix("https://") {
        format!("https://oauth2:{token}@{rest}")
    } else if let Some(rest) = url.strip_prefix("http://") {
        format!("http://oauth2:{token}@{rest}")
    } else {
        url.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_sanitize_name() {
        assert_eq!(sanitize_name("my repo@v2!"), "my-repo-v2-");
        assert_eq!(sanitize_name("normal-name"), "normal-name");
        assert_eq!(sanitize_name("with.dots_and-dashes"), "with.dots_and-dashes");
    }

    #[test]
    fn test_inject_token() {
        assert_eq!(
            inject_token("https://github.com/user/repo.git", "tok123"),
            "https://oauth2:tok123@github.com/user/repo.git"
        );
    }

    #[tokio::test]
    async fn test_list_repos_empty_dir() {
        let tmp = TempDir::new().unwrap();
        let settings = Settings {
            paths: crate::config::PathsConfig {
                base_dir: tmp.path().to_string_lossy().to_string(),
            },
            ..Default::default()
        };
        settings.ensure_directories().unwrap();
        let svc = GitService::new(&settings);
        let repos = svc.list_repos().await.unwrap();
        assert!(repos.is_empty());
    }

    #[tokio::test]
    async fn test_list_repos_with_git_repo() {
        let tmp = TempDir::new().unwrap();
        let settings = Settings {
            paths: crate::config::PathsConfig {
                base_dir: tmp.path().to_string_lossy().to_string(),
            },
            ..Default::default()
        };
        settings.ensure_directories().unwrap();

        // Create a git repo inside repos/
        let repo_dir = settings.repos_dir().join("test-repo");
        std::fs::create_dir_all(&repo_dir).unwrap();
        let result = run("git init", Some(&repo_dir.to_string_lossy())).await;
        assert!(result.success());

        let svc = GitService::new(&settings);
        let repos = svc.list_repos().await.unwrap();
        assert_eq!(repos.len(), 1);
        assert_eq!(repos[0].name, "test-repo");
    }

    #[tokio::test]
    async fn test_get_current_branch() {
        let tmp = TempDir::new().unwrap();
        let repo_dir = tmp.path().join("repo");
        std::fs::create_dir_all(&repo_dir).unwrap();

        // git init + initial commit so HEAD exists
        run("git init", Some(&repo_dir.to_string_lossy())).await;
        run(
            "git commit --allow-empty -m init",
            Some(&repo_dir.to_string_lossy()),
        )
        .await;

        let settings = Settings::default();
        let svc = GitService::new(&settings);
        let branch = svc.get_current_branch(&repo_dir).await;
        // Modern git defaults to "main" or "master"
        assert!(!branch.is_empty());
        assert_ne!(branch, "unknown");
    }

    use crate::config::Settings;

    #[tokio::test]
    async fn test_create_and_list_features() {
        let tmp = TempDir::new().unwrap();
        let settings = Settings {
            paths: crate::config::PathsConfig {
                base_dir: tmp.path().to_string_lossy().to_string(),
            },
            ..Default::default()
        };
        settings.ensure_directories().unwrap();

        // Create a git repo
        let repo_dir = settings.repos_dir().join("test-repo");
        std::fs::create_dir_all(&repo_dir).unwrap();
        run("git init", Some(&repo_dir.to_string_lossy())).await;
        run(
            "git commit --allow-empty -m init",
            Some(&repo_dir.to_string_lossy()),
        )
        .await;

        let svc = GitService::new(&settings);
        let repo_path = repo_dir.to_string_lossy().to_string();

        // Create a feature
        let (wt_path, branch) = svc
            .create_feature(&repo_path, "test-feat", None)
            .await
            .unwrap();
        assert!(wt_path.contains("test-feat"));
        assert_eq!(branch, "feature/test-feat");

        // List features
        let features = svc.list_features(&repo_path).await.unwrap();
        let feat = features.iter().find(|f| f.name == "test-feat");
        assert!(feat.is_some());
        assert!(!feat.unwrap().is_main);

        // Main should be present too
        let main = features.iter().find(|f| f.is_main);
        assert!(main.is_some());
    }

    #[tokio::test]
    async fn test_delete_feature() {
        let tmp = TempDir::new().unwrap();
        let settings = Settings {
            paths: crate::config::PathsConfig {
                base_dir: tmp.path().to_string_lossy().to_string(),
            },
            ..Default::default()
        };
        settings.ensure_directories().unwrap();

        let repo_dir = settings.repos_dir().join("test-repo");
        std::fs::create_dir_all(&repo_dir).unwrap();
        run("git init", Some(&repo_dir.to_string_lossy())).await;
        run(
            "git commit --allow-empty -m init",
            Some(&repo_dir.to_string_lossy()),
        )
        .await;

        let svc = GitService::new(&settings);
        let repo_path = repo_dir.to_string_lossy().to_string();

        // Create then delete
        svc.create_feature(&repo_path, "to-delete", None)
            .await
            .unwrap();
        let deleted = svc.delete_feature(&repo_path, "to-delete").await.unwrap();
        assert!(deleted);
    }
}
