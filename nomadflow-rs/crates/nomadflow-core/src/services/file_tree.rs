use std::path::Path;

use crate::error::{NomadError, Result};
use crate::models::{DirEntry, ListDirResponse};

#[derive(Debug, Clone)]
pub struct FileTreeService;

impl FileTreeService {
    pub fn new() -> Self {
        Self
    }

    pub fn list_dir(&self, worktree_path: &Path, relative_path: &str) -> Result<ListDirResponse> {
        // Canonicalize the worktree root
        let root = worktree_path.canonicalize().map_err(|e| {
            NomadError::NotFound(format!("Worktree path not found: {e}"))
        })?;

        // Determine the target directory
        let rel = relative_path.trim();
        let target = if rel.is_empty() || rel == "." {
            root.clone()
        } else {
            // F2: Reject absolute paths
            if rel.starts_with('/') || rel.starts_with('\\') {
                return Err(NomadError::InvalidInput(
                    "Absolute paths are not allowed".to_string(),
                ));
            }

            let rel_path = Path::new(rel);
            for component in rel_path.components() {
                match component {
                    // Reject parent directory traversal
                    std::path::Component::ParentDir => {
                        return Err(NomadError::InvalidInput(
                            "Path traversal is not allowed".to_string(),
                        ));
                    }
                    // F1: Reject .git directory access
                    std::path::Component::Normal(seg) if seg == ".git" => {
                        return Err(NomadError::InvalidInput(
                            "Access to .git directory is not allowed".to_string(),
                        ));
                    }
                    _ => {}
                }
            }
            root.join(rel)
        };

        // Canonicalize target and verify it's within the worktree root
        let canonical_target = target.canonicalize().map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => {
                NomadError::NotFound(format!("Directory not found: {relative_path}"))
            }
            _ => NomadError::Io(e),
        })?;

        if !canonical_target.starts_with(&root) {
            return Err(NomadError::InvalidInput(
                "Path traversal is not allowed".to_string(),
            ));
        }

        // Read directory entries
        let read_dir = std::fs::read_dir(&canonical_target).map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => {
                NomadError::NotFound(format!("Directory not found: {relative_path}"))
            }
            _ => NomadError::Io(e),
        })?;

        let mut entries: Vec<DirEntry> = Vec::new();

        for entry in read_dir {
            let entry = entry.map_err(NomadError::Io)?;
            let metadata = entry.metadata().map_err(NomadError::Io)?;
            let name = entry.file_name().to_string_lossy().to_string();

            // Skip hidden files/dirs starting with '.'
            if name.starts_with('.') {
                continue;
            }

            let is_dir = metadata.is_dir();
            let size = if is_dir { 0 } else { metadata.len() };

            // Compute relative path from worktree root
            let abs_path = entry.path().canonicalize().unwrap_or_else(|_| entry.path());
            let rel_from_root = abs_path
                .strip_prefix(&root)
                .unwrap_or(&abs_path)
                .to_string_lossy()
                .to_string();

            entries.push(DirEntry {
                name,
                path: rel_from_root,
                is_dir,
                size,
            });
        }

        // Sort: directories first, then alphabetically by name (case-insensitive)
        entries.sort_by(|a, b| {
            b.is_dir
                .cmp(&a.is_dir)
                .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        });

        let echoed_path = if rel.is_empty() || rel == "." {
            String::new()
        } else {
            rel.to_string()
        };

        Ok(ListDirResponse {
            entries,
            path: echoed_path,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn setup_tree() -> TempDir {
        let dir = TempDir::new().unwrap();
        fs::create_dir_all(dir.path().join("src/components")).unwrap();
        fs::write(dir.path().join("src/main.rs"), "fn main() {}").unwrap();
        fs::write(dir.path().join("src/components/app.tsx"), "<App/>").unwrap();
        fs::write(dir.path().join("README.md"), "# Hello").unwrap();
        fs::write(dir.path().join("Cargo.toml"), "[package]").unwrap();
        // Hidden file should be excluded
        fs::write(dir.path().join(".gitignore"), "target/").unwrap();
        dir
    }

    #[test]
    fn test_list_root() {
        let dir = setup_tree();
        let svc = FileTreeService::new();
        let result = svc.list_dir(dir.path(), "").unwrap();

        assert_eq!(result.path, "");
        // Should have: src/ dir, Cargo.toml, README.md (hidden files excluded)
        assert!(result.entries.iter().any(|e| e.name == "src" && e.is_dir));
        assert!(result.entries.iter().any(|e| e.name == "README.md" && !e.is_dir));
        assert!(result.entries.iter().any(|e| e.name == "Cargo.toml" && !e.is_dir));
        // .gitignore should be excluded
        assert!(!result.entries.iter().any(|e| e.name == ".gitignore"));
    }

    #[test]
    fn test_list_subdirectory() {
        let dir = setup_tree();
        let svc = FileTreeService::new();
        let result = svc.list_dir(dir.path(), "src").unwrap();

        assert_eq!(result.path, "src");
        assert!(result.entries.iter().any(|e| e.name == "components" && e.is_dir));
        assert!(result.entries.iter().any(|e| e.name == "main.rs" && !e.is_dir));
    }

    #[test]
    fn test_directories_sorted_first() {
        let dir = setup_tree();
        let svc = FileTreeService::new();
        let result = svc.list_dir(dir.path(), "").unwrap();

        // First entry should be a directory
        if let Some(first) = result.entries.first() {
            assert!(first.is_dir, "Directories should be sorted first");
        }
    }

    #[test]
    fn test_path_traversal_rejected() {
        let dir = setup_tree();
        let svc = FileTreeService::new();
        let result = svc.list_dir(dir.path(), "../../etc");
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("traversal"));
    }

    #[test]
    fn test_nonexistent_path() {
        let dir = setup_tree();
        let svc = FileTreeService::new();
        let result = svc.list_dir(dir.path(), "nonexistent");
        assert!(result.is_err());
        match result.unwrap_err() {
            NomadError::NotFound(_) => {}
            other => panic!("Expected NotFound, got: {other}"),
        }
    }

    #[test]
    fn test_git_directory_rejected() {
        let dir = setup_tree();
        let svc = FileTreeService::new();
        let result = svc.list_dir(dir.path(), ".git");
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains(".git"));
    }

    #[test]
    fn test_git_nested_rejected() {
        let dir = setup_tree();
        let svc = FileTreeService::new();
        let result = svc.list_dir(dir.path(), "src/.git/objects");
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains(".git"));
    }

    #[test]
    fn test_absolute_path_rejected() {
        let dir = setup_tree();
        let svc = FileTreeService::new();
        let result = svc.list_dir(dir.path(), "/etc/passwd");
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Absolute"));
    }

    #[test]
    fn test_file_sizes() {
        let dir = setup_tree();
        let svc = FileTreeService::new();
        let result = svc.list_dir(dir.path(), "").unwrap();

        for entry in &result.entries {
            if entry.is_dir {
                assert_eq!(entry.size, 0, "Directories should have size 0");
            }
        }
        // README.md should have size > 0
        let readme = result.entries.iter().find(|e| e.name == "README.md").unwrap();
        assert!(readme.size > 0);
    }
}
