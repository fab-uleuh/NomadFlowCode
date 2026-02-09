"""Pydantic request models for the NomadFlow API."""

from pydantic import BaseModel, Field, ConfigDict


class ListFeaturesRequest(BaseModel):
    """Request body for listing features of a repository."""

    model_config = ConfigDict(populate_by_name=True)

    repo_path: str = Field(..., alias="repoPath", description="Path to the git repository")


class CreateFeatureRequest(BaseModel):
    """Request body for creating a new feature worktree."""

    model_config = ConfigDict(populate_by_name=True)

    repo_path: str = Field(..., alias="repoPath", description="Path to the git repository")
    feature_name: str = Field(..., alias="featureName", description="Name of the feature to create")
    base_branch: str = Field(
        default="main", alias="baseBranch", description="Branch to base the feature on"
    )


class DeleteFeatureRequest(BaseModel):
    """Request body for deleting a feature worktree."""

    model_config = ConfigDict(populate_by_name=True)

    repo_path: str = Field(..., alias="repoPath", description="Path to the git repository")
    feature_name: str = Field(..., alias="featureName", description="Name of the feature to delete")


class SwitchFeatureRequest(BaseModel):
    """Request body for switching to a feature worktree."""

    model_config = ConfigDict(populate_by_name=True)

    repo_path: str = Field(..., alias="repoPath", description="Path to the git repository")
    feature_name: str = Field(..., alias="featureName", description="Name of the feature to switch to")


class CloneRepoRequest(BaseModel):
    """Request body for cloning a git repository."""

    model_config = ConfigDict(populate_by_name=True)

    url: str = Field(..., description="Git clone URL (HTTPS)")
    token: str | None = Field(default=None, description="Access token for private repos")
    name: str | None = Field(default=None, description="Override repo name (auto-detected from URL if omitted)")
