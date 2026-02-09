"""Git worktree operations service."""

import os
import re
from pathlib import Path
from urllib.parse import urlparse, urlunparse

from ..config import Settings
from ..utils.shell import run_command, CommandResult
from ..models.responses import Repository, Feature


class GitService:
    """Service for managing Git repositories and worktrees."""

    def __init__(self, settings: Settings):
        self.settings = settings
        self.repos_dir = settings.paths.repos_dir
        self.worktrees_dir = settings.paths.worktrees_dir

    async def list_repos(self) -> list[Repository]:
        """List all Git repositories in the repos directory."""
        repos: list[Repository] = []

        if not self.repos_dir.exists():
            return repos

        for item in self.repos_dir.iterdir():
            if item.is_dir():
                git_dir = item / ".git"
                if git_dir.exists():
                    branch = await self._get_current_branch(item)
                    repos.append(
                        Repository(
                            name=item.name,
                            path=str(item),
                            branch=branch,
                        )
                    )

        return repos

    async def clone_repo(
        self, url: str, token: str | None = None, name: str | None = None
    ) -> tuple[str, str, str]:
        """
        Clone a git repository into the repos directory.

        Args:
            url: HTTPS clone URL
            token: Optional access token for private repos
            name: Optional override for the repo directory name

        Returns:
            Tuple of (name, path, branch)
        """
        # Extract repo name from URL if not provided
        if not name:
            path_part = urlparse(url).path.rstrip("/")
            name = Path(path_part).stem  # removes .git extension
            if not name:
                raise ValueError("Cannot determine repository name from URL")

        # Sanitize name
        name = re.sub(r"[^a-zA-Z0-9._-]", "-", name)

        # Check destination doesn't already exist
        dest = self.repos_dir / name
        if dest.exists():
            raise FileExistsError(f"Repository '{name}' already exists")

        # Ensure repos directory exists
        self.repos_dir.mkdir(parents=True, exist_ok=True)

        # Build clone URL with token if provided
        clone_url = url
        if token:
            parsed = urlparse(url)
            clone_url = urlunparse(parsed._replace(
                netloc=f"oauth2:{token}@{parsed.hostname}"
                + (f":{parsed.port}" if parsed.port else "")
            ))

        # Run git clone with long timeout
        result = await run_command(
            f'git clone {clone_url} {dest}',
            timeout=600.0,
        )

        if not result.success:
            raise RuntimeError(f"git clone failed: {result.stderr}")

        # Security: remove token from remote URL
        if token:
            await run_command(
                f'git remote set-url origin {url}',
                cwd=str(dest),
            )

        # Get the default branch
        branch = await self._get_current_branch(dest)

        return name, str(dest), branch

    async def list_features(self, repo_path: str) -> list[Feature]:
        """List all worktrees (features) for a repository."""
        features: list[Feature] = []
        repo_path_obj = Path(repo_path)
        repo_name = repo_path_obj.name

        # Get worktrees from git
        result = await run_command("git worktree list --porcelain", cwd=repo_path)
        if not result.success:
            return features

        # Parse worktree list output
        current_worktree: dict[str, str] = {}
        for line in result.stdout.split("\n"):
            line = line.strip()
            if not line:
                if current_worktree and "worktree" in current_worktree:
                    worktree_path = current_worktree["worktree"]
                    branch = current_worktree.get("branch", "")

                    # Clean up branch name (remove refs/heads/)
                    if branch.startswith("refs/heads/"):
                        branch = branch[len("refs/heads/"):]

                    # Main repository itself: add as is_main=True
                    if worktree_path == str(repo_path_obj):
                        features.append(
                            Feature(
                                name=branch or repo_path_obj.name,
                                worktree_path=worktree_path,
                                branch=branch,
                                is_active=False,
                                is_main=True,
                            )
                        )
                        current_worktree = {}
                        continue

                    # Extract feature name from path
                    feature_name = Path(worktree_path).name

                    features.append(
                        Feature(
                            name=feature_name,
                            worktree_path=worktree_path,
                            branch=branch,
                            is_active=False,
                        )
                    )
                current_worktree = {}
            elif line.startswith("worktree "):
                current_worktree["worktree"] = line[9:]
            elif line.startswith("branch "):
                current_worktree["branch"] = line[7:]

        # Also check the worktrees directory for this repo
        repo_worktrees_dir = self.worktrees_dir / repo_name
        if repo_worktrees_dir.exists():
            existing_paths = {f.worktree_path for f in features}
            for item in repo_worktrees_dir.iterdir():
                if item.is_dir() and str(item) not in existing_paths:
                    branch = await self._get_current_branch(item)
                    features.append(
                        Feature(
                            name=item.name,
                            worktree_path=str(item),
                            branch=branch,
                            is_active=False,
                        )
                    )

        return features

    async def create_feature(
        self, repo_path: str, feature_name: str, base_branch: str | None = None
    ) -> tuple[str, str]:
        """
        Create a new feature worktree.

        Args:
            repo_path: Path to the git repository
            feature_name: Name of the feature (will be used for branch and directory)
            base_branch: Branch to base the feature on (auto-detected if None)

        Returns:
            Tuple of (worktree_path, branch_name)
        """
        repo_path_obj = Path(repo_path)
        repo_name = repo_path_obj.name

        # Auto-detect default branch if not specified
        if not base_branch:
            base_branch = await self._get_default_branch(repo_path)

        # Create worktree directory for this repo
        repo_worktrees_dir = self.worktrees_dir / repo_name
        repo_worktrees_dir.mkdir(parents=True, exist_ok=True)

        worktree_path = repo_worktrees_dir / feature_name
        branch_name = f"feature/{feature_name}"

        # If worktree already exists, just return its path
        if worktree_path.exists():
            return str(worktree_path), branch_name

        # Fetch latest from remote (ignore errors if no remote)
        await run_command("git fetch --all 2>/dev/null || true", cwd=repo_path)

        # Try to create the worktree with a new branch
        result = await run_command(
            f'git worktree add -b "{branch_name}" "{worktree_path}" "{base_branch}"',
            cwd=repo_path,
        )

        if not result.success:
            # Branch might already exist, try to use it
            result = await run_command(
                f'git worktree add "{worktree_path}" "{branch_name}"',
                cwd=repo_path,
            )

        if not result.success:
            # Try with origin/base_branch
            result = await run_command(
                f'git worktree add -b "{branch_name}" "{worktree_path}" "origin/{base_branch}"',
                cwd=repo_path,
            )

        if not result.success:
            # Last resort: create from HEAD
            result = await run_command(
                f'git worktree add -b "{branch_name}" "{worktree_path}" HEAD',
                cwd=repo_path,
            )
            if not result.success:
                raise RuntimeError(f"Failed to create worktree: {result.stderr}")

        return str(worktree_path), branch_name

    async def delete_feature(self, repo_path: str, feature_name: str) -> bool:
        """Delete a feature worktree."""
        repo_path_obj = Path(repo_path)
        repo_name = repo_path_obj.name

        worktree_path = self.worktrees_dir / repo_name / feature_name

        # Remove the worktree
        result = await run_command(
            f'git worktree remove "{worktree_path}" --force',
            cwd=repo_path,
        )

        if not result.success:
            # Try to prune if removal fails
            await run_command("git worktree prune", cwd=repo_path)

            # Delete directory manually if it still exists
            if worktree_path.exists():
                import shutil
                shutil.rmtree(worktree_path, ignore_errors=True)

        # Optionally delete the branch
        branch_name = f"feature/{feature_name}"
        await run_command(
            f'git branch -D "{branch_name}"',
            cwd=repo_path,
        )

        return True

    async def _get_current_branch(self, repo_path: Path) -> str:
        """Get the current branch of a repository."""
        result = await run_command(
            "git rev-parse --abbrev-ref HEAD",
            cwd=str(repo_path),
        )
        if result.success:
            return result.stdout.strip()
        return "unknown"

    async def _get_default_branch(self, repo_path: str) -> str:
        """Get the default branch of a repository (main, master, etc.)."""
        # Try to get from remote HEAD
        result = await run_command(
            "git symbolic-ref refs/remotes/origin/HEAD 2>/dev/null | sed 's@^refs/remotes/origin/@@'",
            cwd=repo_path,
        )
        if result.success and result.stdout.strip():
            return result.stdout.strip()

        # Check if common branches exist
        for branch in ["main", "master", "develop", "dev"]:
            result = await run_command(
                f'git rev-parse --verify "{branch}" 2>/dev/null',
                cwd=repo_path,
            )
            if result.success:
                return branch

        # Fall back to current branch
        result = await run_command(
            "git rev-parse --abbrev-ref HEAD",
            cwd=repo_path,
        )
        if result.success:
            return result.stdout.strip()

        return "main"
