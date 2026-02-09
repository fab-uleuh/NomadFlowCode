"""Configuration management using pydantic-settings and TOML."""

import os
from pathlib import Path
from functools import lru_cache

import toml
from pydantic import Field
from pydantic_settings import BaseSettings


class PathsConfig(BaseSettings):
    """Paths configuration."""

    base_dir: str = Field(default="~/.nomadflowcode", description="Base directory for NomadFlow data")

    @property
    def expanded_base_dir(self) -> Path:
        """Return the expanded base directory path."""
        return Path(self.base_dir).expanduser()

    @property
    def repos_dir(self) -> Path:
        """Return the repos directory path."""
        return self.expanded_base_dir / "repos"

    @property
    def worktrees_dir(self) -> Path:
        """Return the worktrees directory path."""
        return self.expanded_base_dir / "worktrees"


class TmuxConfig(BaseSettings):
    """Tmux configuration."""

    session: str = Field(default="nomadflow", description="Tmux session name")


class TtydConfig(BaseSettings):
    """Ttyd configuration."""

    port: int = Field(default=7681, description="Port for ttyd")


class AuthConfig(BaseSettings):
    """Authentication configuration."""

    secret: str = Field(default="", description="Shared secret for API and ttyd authentication")


class ApiConfig(BaseSettings):
    """API configuration."""

    port: int = Field(default=8080, description="Port for the API server")
    host: str = Field(default="0.0.0.0", description="Host to bind the API server")


class Settings(BaseSettings):
    """Main settings class combining all configurations."""

    paths: PathsConfig = Field(default_factory=PathsConfig)
    tmux: TmuxConfig = Field(default_factory=TmuxConfig)
    ttyd: TtydConfig = Field(default_factory=TtydConfig)
    api: ApiConfig = Field(default_factory=ApiConfig)
    auth: AuthConfig = Field(default_factory=AuthConfig)

    @classmethod
    def from_toml(cls, config_path: Path | None = None) -> "Settings":
        """Load settings from a TOML configuration file."""
        if config_path is None:
            config_path = Path("~/.nomadflowcode/config.toml").expanduser()

        if config_path.exists():
            with open(config_path) as f:
                config_data = toml.load(f)

            return cls(
                paths=PathsConfig(**config_data.get("paths", {})),
                tmux=TmuxConfig(**config_data.get("tmux", {})),
                ttyd=TtydConfig(**config_data.get("ttyd", {})),
                api=ApiConfig(**config_data.get("api", {})),
                auth=AuthConfig(**config_data.get("auth", {})),
            )

        return cls()

    def ensure_directories(self) -> None:
        """Create necessary directories if they don't exist."""
        self.paths.expanded_base_dir.mkdir(parents=True, exist_ok=True)
        self.paths.repos_dir.mkdir(parents=True, exist_ok=True)
        self.paths.worktrees_dir.mkdir(parents=True, exist_ok=True)

    def save_default_config(self) -> None:
        """Save a default configuration file if it doesn't exist."""
        config_path = self.paths.expanded_base_dir / "config.toml"
        if not config_path.exists():
            self.ensure_directories()
            default_config = """[paths]
base_dir = "~/.nomadflowcode"

[tmux]
session = "nomadflow"

[ttyd]
port = 7681

[api]
port = 8080

# Authentication - uncomment and set a secret to protect API and terminal
# The same secret must be entered in the mobile app
# [auth]
# secret = "your-secret-here"
"""
            with open(config_path, "w") as f:
                f.write(default_config)


@lru_cache
def get_settings() -> Settings:
    """Get cached settings instance."""
    settings = Settings.from_toml()
    settings.ensure_directories()
    return settings
