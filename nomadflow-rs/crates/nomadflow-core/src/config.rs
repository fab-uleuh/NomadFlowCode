use std::path::PathBuf;

use serde::Deserialize;

use crate::error::{NomadError, Result};

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct PathsConfig {
    pub base_dir: String,
}

impl Default for PathsConfig {
    fn default() -> Self {
        Self {
            base_dir: "~/.nomadflowcode".to_string(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct TmuxConfig {
    pub session: String,
}

impl Default for TmuxConfig {
    fn default() -> Self {
        Self {
            session: "nomadflow".to_string(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct TtydConfig {
    pub port: u16,
}

impl Default for TtydConfig {
    fn default() -> Self {
        Self { port: 7681 }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct ApiConfig {
    pub port: u16,
    pub host: String,
}

impl Default for ApiConfig {
    fn default() -> Self {
        Self {
            port: 8080,
            host: "0.0.0.0".to_string(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct AuthConfig {
    pub secret: String,
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            secret: String::new(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub paths: PathsConfig,
    pub tmux: TmuxConfig,
    pub ttyd: TtydConfig,
    pub api: ApiConfig,
    pub auth: AuthConfig,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            paths: PathsConfig::default(),
            tmux: TmuxConfig::default(),
            ttyd: TtydConfig::default(),
            api: ApiConfig::default(),
            auth: AuthConfig::default(),
        }
    }
}

impl Settings {
    /// Expand `~` to the user's home directory.
    fn expand_home(path: &str) -> PathBuf {
        if path.starts_with('~') {
            if let Some(home) = dirs::home_dir() {
                return home.join(&path[2..]);
            }
        }
        PathBuf::from(path)
    }

    /// Expanded base directory.
    pub fn base_dir(&self) -> PathBuf {
        Self::expand_home(&self.paths.base_dir)
    }

    /// Repos directory.
    pub fn repos_dir(&self) -> PathBuf {
        self.base_dir().join("repos")
    }

    /// Worktrees directory.
    pub fn worktrees_dir(&self) -> PathBuf {
        self.base_dir().join("worktrees")
    }

    /// Load settings from the TOML config file.
    pub fn load(config_path: Option<&PathBuf>) -> Result<Self> {
        let path = match config_path {
            Some(p) => p.clone(),
            None => Self::expand_home("~/.nomadflowcode/config.toml"),
        };

        if path.exists() {
            let content = std::fs::read_to_string(&path)
                .map_err(|e| NomadError::Config(format!("Failed to read config: {e}")))?;
            let settings: Settings = toml::from_str(&content)
                .map_err(|e| NomadError::Config(format!("Failed to parse config: {e}")))?;
            Ok(settings)
        } else {
            Ok(Settings::default())
        }
    }

    /// Create necessary directories if they don't exist.
    pub fn ensure_directories(&self) -> Result<()> {
        std::fs::create_dir_all(self.base_dir())?;
        std::fs::create_dir_all(self.repos_dir())?;
        std::fs::create_dir_all(self.worktrees_dir())?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_parse_full_toml() {
        let toml_str = r#"
[paths]
base_dir = "/tmp/nomadtest"

[tmux]
session = "mytest"

[ttyd]
port = 9999

[api]
port = 3000
host = "127.0.0.1"

[auth]
secret = "s3cret"
"#;
        let settings: Settings = toml::from_str(toml_str).unwrap();
        assert_eq!(settings.paths.base_dir, "/tmp/nomadtest");
        assert_eq!(settings.tmux.session, "mytest");
        assert_eq!(settings.ttyd.port, 9999);
        assert_eq!(settings.api.port, 3000);
        assert_eq!(settings.api.host, "127.0.0.1");
        assert_eq!(settings.auth.secret, "s3cret");
    }

    #[test]
    fn test_parse_minimal_toml() {
        let toml_str = "";
        let settings: Settings = toml::from_str(toml_str).unwrap();
        assert_eq!(settings.paths.base_dir, "~/.nomadflowcode");
        assert_eq!(settings.tmux.session, "nomadflow");
        assert_eq!(settings.ttyd.port, 7681);
        assert_eq!(settings.api.port, 8080);
        assert_eq!(settings.auth.secret, "");
    }

    #[test]
    fn test_expand_home() {
        let path = Settings::expand_home("~/test");
        assert!(path.is_absolute());
        assert!(path.to_str().unwrap().ends_with("/test"));
    }

    #[test]
    fn test_invalid_toml() {
        let result = toml::from_str::<Settings>("{{invalid");
        assert!(result.is_err());
    }

    #[test]
    fn test_ensure_directories() {
        let tmp = TempDir::new().unwrap();
        let base = tmp.path().join("nomadtest");
        let settings = Settings {
            paths: PathsConfig {
                base_dir: base.to_str().unwrap().to_string(),
            },
            ..Default::default()
        };
        settings.ensure_directories().unwrap();
        assert!(base.exists());
        assert!(base.join("repos").exists());
        assert!(base.join("worktrees").exists());
    }
}
