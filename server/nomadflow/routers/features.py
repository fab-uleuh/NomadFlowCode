"""Feature (worktree) management endpoints."""

from pathlib import Path

from fastapi import APIRouter, Depends, HTTPException

from ..config import Settings, get_settings
from ..auth import verify_auth
from ..services.git_service import GitService
from ..services.tmux_service import TmuxService
from ..models.requests import (
    ListFeaturesRequest,
    CreateFeatureRequest,
    DeleteFeatureRequest,
    SwitchFeatureRequest,
)
from ..models.responses import (
    ListFeaturesResponse,
    CreateFeatureResponse,
    DeleteFeatureResponse,
    SwitchFeatureResponse,
)

router = APIRouter(prefix="/api", tags=["features"], dependencies=[Depends(verify_auth)])


def get_git_service(settings: Settings = Depends(get_settings)) -> GitService:
    """Dependency to get GitService instance."""
    return GitService(settings)


def get_tmux_service(settings: Settings = Depends(get_settings)) -> TmuxService:
    """Dependency to get TmuxService instance."""
    return TmuxService(settings)


def get_window_name(repo_path: str, feature_name: str) -> str:
    """Build tmux window name from repo and feature.

    Format: repo:feature (e.g., 'my-project:add-login')
    This prevents collisions when different repos have features with the same name.
    """
    repo_name = Path(repo_path).name
    return f"{repo_name}:{feature_name}"


@router.post("/list-features", response_model=ListFeaturesResponse)
async def list_features(
    request: ListFeaturesRequest,
    git_service: GitService = Depends(get_git_service),
) -> ListFeaturesResponse:
    """
    List all features (worktrees) for a repository.

    Returns a list of features with their names, worktree paths, branches,
    and active status.
    """
    try:
        features = await git_service.list_features(request.repo_path)
        return ListFeaturesResponse(features=features)
    except Exception as e:
        raise HTTPException(status_code=500, detail=str(e))


@router.post("/create-feature", response_model=CreateFeatureResponse)
async def create_feature(
    request: CreateFeatureRequest,
    git_service: GitService = Depends(get_git_service),
    tmux_service: TmuxService = Depends(get_tmux_service),
) -> CreateFeatureResponse:
    """
    Create a new feature worktree.

    Creates a git worktree for the feature and sets up a tmux window
    for working on it.
    """
    try:
        # Create the git worktree (auto-detects base branch if not specified or "main")
        base_branch = request.base_branch if request.base_branch != "main" else None
        worktree_path, branch = await git_service.create_feature(
            request.repo_path,
            request.feature_name,
            base_branch,
        )

        # Ensure tmux session exists
        await tmux_service.ensure_session()

        # Create or ensure tmux window for this feature
        # Window name format: repo:feature to avoid collisions
        window_name = get_window_name(request.repo_path, request.feature_name)
        await tmux_service.ensure_window(window_name, worktree_path)

        return CreateFeatureResponse(
            worktree_path=worktree_path,
            branch=branch,
            tmux_window=window_name,
        )
    except RuntimeError as e:
        raise HTTPException(status_code=400, detail=str(e))
    except Exception as e:
        raise HTTPException(status_code=500, detail=str(e))


@router.post("/delete-feature", response_model=DeleteFeatureResponse)
async def delete_feature(
    request: DeleteFeatureRequest,
    git_service: GitService = Depends(get_git_service),
    tmux_service: TmuxService = Depends(get_tmux_service),
) -> DeleteFeatureResponse:
    """
    Delete a feature worktree.

    Removes the git worktree and optionally the associated branch.
    Also kills any associated tmux window.
    """
    try:
        # Kill tmux window if it exists
        # Window name format: repo:feature
        window_name = get_window_name(request.repo_path, request.feature_name)
        await tmux_service.kill_window(window_name)

        # Delete the git worktree
        deleted = await git_service.delete_feature(
            request.repo_path,
            request.feature_name,
        )

        return DeleteFeatureResponse(deleted=deleted)
    except Exception as e:
        raise HTTPException(status_code=500, detail=str(e))


@router.post("/switch-feature", response_model=SwitchFeatureResponse)
async def switch_feature(
    request: SwitchFeatureRequest,
    git_service: GitService = Depends(get_git_service),
    tmux_service: TmuxService = Depends(get_tmux_service),
) -> SwitchFeatureResponse:
    """
    Switch to a feature worktree.

    This endpoint:
    1. Finds or creates the feature's worktree
    2. Ensures the tmux session exists
    3. Creates/selects the tmux window for the feature
    4. CDs into the worktree directory
    5. Clears the terminal

    After calling this endpoint, the ttyd terminal will show the active
    tmux window positioned in the correct worktree directory.
    """
    try:
        # Get features to find the worktree path
        features = await git_service.list_features(request.repo_path)
        feature = next(
            (f for f in features if f.name == request.feature_name),
            None,
        )

        worktree_path: str

        if not feature:
            # Feature doesn't exist, create it automatically
            # create_feature auto-detects the default branch
            worktree_path, branch = await git_service.create_feature(
                request.repo_path,
                request.feature_name,
                None,  # Auto-detect base branch
            )
        else:
            worktree_path = feature.worktree_path

        # Ensure tmux session exists
        await tmux_service.ensure_session()

        # Switch to window (creates if needed, selects, cds, and clears)
        # Returns (switched, has_running_process) - won't send cd/clear if process is running
        # Window name format: repo:feature to avoid collisions
        window_name = get_window_name(request.repo_path, request.feature_name)
        switched, has_running_process = await tmux_service.switch_to_window(
            window_name,
            worktree_path,
        )

        if not switched:
            raise HTTPException(
                status_code=500,
                detail=f"Failed to switch to window '{window_name}'",
            )

        return SwitchFeatureResponse(
            switched=True,
            worktree_path=worktree_path,
            tmux_window=window_name,
            has_running_process=has_running_process,
        )
    except RuntimeError as e:
        raise HTTPException(status_code=400, detail=str(e))
    except HTTPException:
        raise
    except Exception as e:
        raise HTTPException(status_code=500, detail=str(e))
