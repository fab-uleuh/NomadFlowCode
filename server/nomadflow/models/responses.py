"""Pydantic response models for the NomadFlow API."""

from pydantic import BaseModel, Field


class Repository(BaseModel):
    """Repository information."""

    name: str = Field(..., description="Repository name")
    path: str = Field(..., description="Full path to the repository")
    branch: str = Field(..., description="Current branch name")


class Feature(BaseModel):
    """Feature worktree information."""

    name: str = Field(..., description="Feature name")
    worktree_path: str = Field(..., alias="worktreePath", description="Path to the worktree")
    branch: str = Field(..., description="Branch name")
    is_active: bool = Field(default=False, alias="isActive", description="Whether this feature is currently active")

    model_config = {"populate_by_name": True}


class CloneRepoResponse(BaseModel):
    """Response for cloning a repository."""

    name: str = Field(..., description="Repository name")
    path: str = Field(..., description="Full path to the cloned repository")
    branch: str = Field(..., description="Default branch name")


class ListReposResponse(BaseModel):
    """Response for listing repositories."""

    repos: list[Repository] = Field(default_factory=list, description="List of repositories")


class ListFeaturesResponse(BaseModel):
    """Response for listing features of a repository."""

    features: list[Feature] = Field(default_factory=list, description="List of features")


class CreateFeatureResponse(BaseModel):
    """Response for creating a feature."""

    worktree_path: str = Field(..., alias="worktreePath", description="Path to the created worktree")
    branch: str = Field(..., description="Branch name")
    tmux_window: str = Field(..., alias="tmuxWindow", description="Name of the tmux window")

    model_config = {"populate_by_name": True}


class DeleteFeatureResponse(BaseModel):
    """Response for deleting a feature."""

    deleted: bool = Field(..., description="Whether the feature was deleted")


class SwitchFeatureResponse(BaseModel):
    """Response for switching to a feature."""

    switched: bool = Field(..., description="Whether the switch was successful")
    worktree_path: str = Field(..., alias="worktreePath", description="Path to the worktree")
    tmux_window: str = Field(..., alias="tmuxWindow", description="Name of the tmux window")
    has_running_process: bool = Field(
        default=False,
        alias="hasRunningProcess",
        description="Whether an interactive process (like claude) is already running in the terminal",
    )

    model_config = {"populate_by_name": True}
