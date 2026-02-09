"""Repository management endpoints."""

from fastapi import APIRouter, Depends, HTTPException

from ..config import Settings, get_settings
from ..auth import verify_auth
from ..services.git_service import GitService
from ..models.requests import CloneRepoRequest
from ..models.responses import ListReposResponse, CloneRepoResponse

router = APIRouter(prefix="/api", tags=["repos"], dependencies=[Depends(verify_auth)])


def get_git_service(settings: Settings = Depends(get_settings)) -> GitService:
    """Dependency to get GitService instance."""
    return GitService(settings)


@router.post("/list-repos", response_model=ListReposResponse)
async def list_repos(
    git_service: GitService = Depends(get_git_service),
) -> ListReposResponse:
    """
    List all Git repositories in the configured repos directory.

    Returns a list of repositories with their names, paths, and current branches.
    """
    try:
        repos = await git_service.list_repos()
        return ListReposResponse(repos=repos)
    except Exception as e:
        raise HTTPException(status_code=500, detail=str(e))


@router.post("/clone-repo", response_model=CloneRepoResponse)
async def clone_repo(
    request: CloneRepoRequest,
    git_service: GitService = Depends(get_git_service),
) -> CloneRepoResponse:
    """Clone a git repository into the repos directory."""
    try:
        name, path, branch = await git_service.clone_repo(
            url=request.url,
            token=request.token,
            name=request.name,
        )
        return CloneRepoResponse(name=name, path=path, branch=branch)
    except FileExistsError as e:
        raise HTTPException(status_code=409, detail=str(e))
    except ValueError as e:
        raise HTTPException(status_code=400, detail=str(e))
    except Exception as e:
        raise HTTPException(status_code=500, detail=str(e))
