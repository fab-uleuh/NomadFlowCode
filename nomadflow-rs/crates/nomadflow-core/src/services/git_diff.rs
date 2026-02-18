use std::path::Path;

use git2::{DiffOptions, Patch, Repository, StatusOptions};

use crate::error::{NomadError, Result};
use crate::models::{
    DiffHunk, DiffLine, FileChange, FileDiffResponse, StatusSummary, WorktreeStatusResponse,
};

#[derive(Debug, Clone)]
pub struct GitDiffService;

impl GitDiffService {
    pub fn new() -> Self {
        Self
    }

    pub fn worktree_status(&self, worktree_path: &Path) -> Result<WorktreeStatusResponse> {
        let repo = Repository::open(worktree_path)?;

        // Get current branch name
        let branch = repo
            .head()
            .ok()
            .and_then(|h| h.shorthand().map(|s| s.to_string()))
            .unwrap_or_else(|| "HEAD".to_string());

        // Get file statuses
        let mut opts = StatusOptions::new();
        opts.include_untracked(true)
            .recurse_untracked_dirs(true)
            .include_ignored(false)
            .exclude_submodules(true);

        let statuses = repo.statuses(Some(&mut opts))?;

        // Build a set of files with their status strings
        let mut files: Vec<FileChange> = Vec::new();
        let mut modified_count = 0usize;
        let mut new_count = 0usize;
        let mut deleted_count = 0usize;
        let mut conflicted_count = 0usize;

        for entry in statuses.iter() {
            let path = entry.path().unwrap_or("").to_string();
            let st = entry.status();

            let status_str = if st.intersects(
                git2::Status::INDEX_DELETED | git2::Status::WT_DELETED,
            ) {
                "deleted"
            } else if st.intersects(
                git2::Status::INDEX_NEW | git2::Status::WT_NEW,
            ) {
                "new"
            } else if st.intersects(
                git2::Status::INDEX_MODIFIED | git2::Status::WT_MODIFIED,
            ) {
                "modified"
            } else if st.intersects(
                git2::Status::INDEX_RENAMED | git2::Status::WT_RENAMED,
            ) {
                "renamed"
            } else if st.intersects(git2::Status::CONFLICTED) {
                "conflicted"
            } else {
                continue;
            };

            match status_str {
                "modified" | "renamed" => modified_count += 1,
                "new" => new_count += 1,
                "deleted" => deleted_count += 1,
                "conflicted" => conflicted_count += 1,
                _ => {}
            }

            files.push(FileChange {
                path,
                status: status_str.to_string(),
                additions: 0,
                deletions: 0,
            });
        }

        // Compute per-file additions/deletions via diff
        // Unstaged changes (workdir vs index) — include untracked files for line counts
        let mut unstaged_opts = DiffOptions::new();
        unstaged_opts.include_untracked(true);
        unstaged_opts.show_untracked_content(true);
        unstaged_opts.recurse_untracked_dirs(true);
        let diff_unstaged = repo.diff_index_to_workdir(None, Some(&mut unstaged_opts))?;
        apply_line_stats(&diff_unstaged, &mut files);

        // Staged changes (index vs HEAD)
        let head_tree = repo
            .head()
            .ok()
            .and_then(|h| h.peel_to_tree().ok());
        let diff_staged = repo.diff_tree_to_index(
            head_tree.as_ref(),
            Some(&repo.index()?),
            None,
        )?;
        apply_line_stats(&diff_staged, &mut files);

        let total_additions: usize = files.iter().map(|f| f.additions).sum();
        let total_deletions: usize = files.iter().map(|f| f.deletions).sum();

        Ok(WorktreeStatusResponse {
            worktree_path: worktree_path.to_string_lossy().to_string(),
            branch,
            files,
            summary: StatusSummary {
                modified: modified_count,
                new: new_count,
                deleted: deleted_count,
                conflicted: conflicted_count,
                total_additions,
                total_deletions,
            },
        })
    }

    pub fn file_diff(&self, worktree_path: &Path, file_path: &str) -> Result<FileDiffResponse> {
        let repo = Repository::open(worktree_path)?;

        let mut hunks: Vec<DiffHunk> = Vec::new();

        // Unstaged changes (workdir vs index)
        let mut diff_opts = DiffOptions::new();
        diff_opts.pathspec(file_path);
        let diff = repo.diff_index_to_workdir(None, Some(&mut diff_opts))?;
        extract_hunks_from_diff(&diff, &mut hunks);

        // Staged changes (index vs HEAD)
        let head_tree = repo.head().ok().and_then(|h| h.peel_to_tree().ok());
        let mut staged_opts = DiffOptions::new();
        staged_opts.pathspec(file_path);
        let staged_diff = repo.diff_tree_to_index(
            head_tree.as_ref(),
            Some(&repo.index()?),
            Some(&mut staged_opts),
        )?;
        extract_hunks_from_diff(&staged_diff, &mut hunks);

        Ok(FileDiffResponse {
            file_path: file_path.to_string(),
            hunks,
        })
    }

    pub fn file_content(&self, worktree_path: &Path, file_path: &str) -> Result<String> {
        // Path traversal prevention
        let file_path_obj = Path::new(file_path);

        // Reject absolute paths
        if file_path_obj.is_absolute() {
            return Err(NomadError::InvalidInput(
                "Absolute paths are not allowed".to_string(),
            ));
        }

        // Reject path traversal and .git directory access via component inspection
        for component in file_path_obj.components() {
            match component {
                std::path::Component::ParentDir => {
                    return Err(NomadError::InvalidInput(
                        "Path traversal is not allowed".to_string(),
                    ));
                }
                std::path::Component::Normal(s) if s == ".git" => {
                    return Err(NomadError::InvalidInput(
                        "Access to .git directory is not allowed".to_string(),
                    ));
                }
                _ => {}
            }
        }

        let repo = Repository::open(worktree_path)?;
        let workdir = repo.workdir().ok_or_else(|| {
            NomadError::Other("Repository has no working directory".to_string())
        })?;

        let target = workdir.join(file_path);

        // Canonicalize both paths to verify the target is within the worktree
        let canonical_workdir = workdir.canonicalize().map_err(|e| {
            NomadError::Other(format!("Cannot resolve worktree path: {e}"))
        })?;
        let canonical_target = target.canonicalize().map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => {
                NomadError::NotFound(format!("File not found: {file_path}"))
            }
            _ => NomadError::Io(e),
        })?;

        if !canonical_target.starts_with(&canonical_workdir) {
            return Err(NomadError::InvalidInput(
                "Path traversal is not allowed".to_string(),
            ));
        }

        std::fs::read_to_string(&canonical_target).map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => {
                NomadError::NotFound(format!("File not found: {file_path}"))
            }
            _ => NomadError::Io(e),
        })
    }
}

/// Extract hunks from a diff using Patch objects (avoids borrow issues with Diff::foreach).
fn extract_hunks_from_diff(diff: &git2::Diff, hunks: &mut Vec<DiffHunk>) {
    for idx in 0..diff.deltas().len() {
        if let Ok(Some(patch)) = Patch::from_diff(diff, idx) {
            for hunk_idx in 0..patch.num_hunks() {
                if let Ok((hunk, _lines_in_hunk)) = patch.hunk(hunk_idx) {
                    let mut lines = Vec::new();
                    let num_lines = patch.num_lines_in_hunk(hunk_idx).unwrap_or(0);
                    for line_idx in 0..num_lines {
                        if let Ok(line) = patch.line_in_hunk(hunk_idx, line_idx) {
                            let line_type = match line.origin() {
                                '+' => "addition",
                                '-' => "deletion",
                                ' ' => "context",
                                _ => continue,
                            };
                            let content = String::from_utf8_lossy(line.content())
                                .trim_end_matches('\n')
                                .trim_end_matches('\r')
                                .to_string();
                            lines.push(DiffLine {
                                line_type: line_type.to_string(),
                                content,
                            });
                        }
                    }
                    hunks.push(DiffHunk {
                        old_start: hunk.old_start(),
                        old_lines: hunk.old_lines(),
                        new_start: hunk.new_start(),
                        new_lines: hunk.new_lines(),
                        lines,
                    });
                }
            }
        }
    }
}

/// Apply line stats from a diff to the files vector.
fn apply_line_stats(diff: &git2::Diff, files: &mut [FileChange]) {
    for idx in 0..diff.deltas().len() {
        if let Ok(Some(patch)) = git2::Patch::from_diff(diff, idx) {
            if let Ok((_context, additions, deletions)) = patch.line_stats() {
                let delta = diff.get_delta(idx);
                if let Some(delta) = delta {
                    let diff_path = delta
                        .new_file()
                        .path()
                        .or_else(|| delta.old_file().path())
                        .map(|p| p.to_string_lossy().to_string());

                    if let Some(diff_path) = diff_path {
                        if let Some(file) = files.iter_mut().find(|f| f.path == diff_path) {
                            file.additions += additions;
                            file.deletions += deletions;
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use git2::Signature;
    use std::fs;
    use tempfile::TempDir;

    /// Helper: create a temp git repo with an initial commit
    fn setup_repo() -> (TempDir, Repository) {
        let dir = TempDir::new().unwrap();
        let repo = Repository::init(dir.path()).unwrap();

        // Configure user for commits
        {
            let mut config = repo.config().unwrap();
            config.set_str("user.name", "Test").unwrap();
            config.set_str("user.email", "test@test.com").unwrap();
        }

        // Create initial commit with a file
        fs::write(dir.path().join("hello.txt"), "Hello, world!\n").unwrap();
        {
            let mut index = repo.index().unwrap();
            index.add_path(Path::new("hello.txt")).unwrap();
            index.write().unwrap();
            let tree_id = index.write_tree().unwrap();
            let tree = repo.find_tree(tree_id).unwrap();
            let sig = Signature::now("Test", "test@test.com").unwrap();
            repo.commit(Some("HEAD"), &sig, &sig, "initial commit", &tree, &[])
                .unwrap();
        }

        (dir, repo)
    }

    #[test]
    fn test_worktree_status_modified_file() {
        let (dir, _repo) = setup_repo();
        fs::write(dir.path().join("hello.txt"), "Hello, modified!\n").unwrap();

        let svc = GitDiffService::new();
        let result = svc.worktree_status(dir.path()).unwrap();

        assert_eq!(result.files.len(), 1);
        assert_eq!(result.files[0].path, "hello.txt");
        assert_eq!(result.files[0].status, "modified");
        assert!(result.files[0].additions > 0 || result.files[0].deletions > 0);
    }

    #[test]
    fn test_worktree_status_new_file() {
        let (dir, _repo) = setup_repo();
        fs::write(dir.path().join("new_file.txt"), "I am new\n").unwrap();

        let svc = GitDiffService::new();
        let result = svc.worktree_status(dir.path()).unwrap();

        let new_file = result.files.iter().find(|f| f.path == "new_file.txt");
        assert!(new_file.is_some());
        let new_file = new_file.unwrap();
        assert_eq!(new_file.status, "new");
        assert!(
            new_file.additions > 0,
            "new files should have additions counted"
        );
    }

    #[test]
    fn test_worktree_status_deleted_file() {
        let (dir, _repo) = setup_repo();
        fs::remove_file(dir.path().join("hello.txt")).unwrap();

        let svc = GitDiffService::new();
        let result = svc.worktree_status(dir.path()).unwrap();

        assert_eq!(result.files.len(), 1);
        assert_eq!(result.files[0].path, "hello.txt");
        assert_eq!(result.files[0].status, "deleted");
    }

    #[test]
    fn test_worktree_status_clean_repo() {
        let (dir, _repo) = setup_repo();

        let svc = GitDiffService::new();
        let result = svc.worktree_status(dir.path()).unwrap();

        assert!(result.files.is_empty());
    }

    #[test]
    fn test_worktree_status_summary_counts() {
        let (dir, _repo) = setup_repo();

        // Modify existing file
        fs::write(dir.path().join("hello.txt"), "modified\n").unwrap();
        // Add 2 new files
        fs::write(dir.path().join("new1.txt"), "new1\n").unwrap();
        fs::write(dir.path().join("new2.txt"), "new2\n").unwrap();

        let svc = GitDiffService::new();
        let result = svc.worktree_status(dir.path()).unwrap();

        assert_eq!(result.summary.modified, 1);
        assert_eq!(result.summary.new, 2);
        assert_eq!(result.summary.deleted, 0);
        assert_eq!(result.files.len(), 3);
    }

    #[test]
    fn test_worktree_status_branch_name() {
        let (dir, repo) = setup_repo();

        // Create and checkout a new branch
        let head = repo.head().unwrap().peel_to_commit().unwrap();
        repo.branch("feature-test", &head, false).unwrap();
        repo.set_head("refs/heads/feature-test").unwrap();

        let svc = GitDiffService::new();
        let result = svc.worktree_status(dir.path()).unwrap();
        assert_eq!(result.branch, "feature-test");
    }

    #[test]
    fn test_file_diff_returns_hunks() {
        let (dir, _repo) = setup_repo();
        fs::write(
            dir.path().join("hello.txt"),
            "Hello, world!\nNew line added\n",
        )
        .unwrap();

        let svc = GitDiffService::new();
        let result = svc.file_diff(dir.path(), "hello.txt").unwrap();

        assert!(!result.hunks.is_empty());
        let hunk = &result.hunks[0];
        assert!(!hunk.lines.is_empty());

        // Should have at least one addition
        let has_addition = hunk.lines.iter().any(|l| l.line_type == "addition");
        assert!(has_addition);
    }

    #[test]
    fn test_file_diff_nonexistent_file() {
        let (dir, _repo) = setup_repo();

        let svc = GitDiffService::new();
        let result = svc.file_diff(dir.path(), "nonexistent.txt").unwrap();

        assert!(result.hunks.is_empty());
    }

    #[test]
    fn test_file_content_reads_file() {
        let (dir, _repo) = setup_repo();

        let svc = GitDiffService::new();
        let content = svc.file_content(dir.path(), "hello.txt").unwrap();
        assert_eq!(content, "Hello, world!\n");
    }

    #[test]
    fn test_file_content_path_traversal_rejected() {
        let (dir, _repo) = setup_repo();

        let svc = GitDiffService::new();
        let result = svc.file_content(dir.path(), "../../../etc/passwd");
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("traversal"));
    }

    #[test]
    fn test_file_content_absolute_path_rejected() {
        let (dir, _repo) = setup_repo();

        let svc = GitDiffService::new();
        let result = svc.file_content(dir.path(), "/etc/passwd");
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Absolute"));
    }

    #[test]
    fn test_file_content_dotgit_rejected() {
        let (dir, _repo) = setup_repo();

        let svc = GitDiffService::new();
        let result = svc.file_content(dir.path(), ".git/config");
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains(".git"));
    }

    #[test]
    fn test_file_content_nonexistent_file() {
        let (dir, _repo) = setup_repo();

        let svc = GitDiffService::new();
        let result = svc.file_content(dir.path(), "does_not_exist.txt");
        assert!(result.is_err());
        match result.unwrap_err() {
            NomadError::NotFound(_) => {} // expected
            other => panic!("Expected NotFound, got: {other}"),
        }
    }

    #[test]
    fn test_file_content_gitignore_allowed() {
        let (dir, _repo) = setup_repo();
        fs::write(dir.path().join(".gitignore"), "*.tmp\n").unwrap();

        let svc = GitDiffService::new();
        let content = svc.file_content(dir.path(), ".gitignore").unwrap();
        assert_eq!(content, "*.tmp\n");
    }

    #[test]
    fn test_file_content_github_dir_allowed() {
        let (dir, _repo) = setup_repo();
        fs::create_dir_all(dir.path().join(".github").join("workflows")).unwrap();
        fs::write(
            dir.path().join(".github").join("workflows").join("ci.yml"),
            "name: CI\n",
        )
        .unwrap();

        let svc = GitDiffService::new();
        let content = svc
            .file_content(dir.path(), ".github/workflows/ci.yml")
            .unwrap();
        assert_eq!(content, "name: CI\n");
    }
}
