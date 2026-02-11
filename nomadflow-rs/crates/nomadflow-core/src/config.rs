use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::error::{NomadError, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TtydConfig {
    pub port: u16,
}

impl Default for TtydConfig {
    fn default() -> Self {
        Self { port: 7681 }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct AuthConfig {
    pub secret: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TunnelConfig {
    pub relay_host: String,
    pub relay_port: u16,
    pub relay_secret: String,
    /// Preferred subdomain for stable public URL. Empty = random (default).
    pub subdomain: String,
}

impl Default for TunnelConfig {
    fn default() -> Self {
        Self {
            relay_host: "relay.nomadflowcode.dev".to_string(),
            relay_port: 7835,
            // Public shared secret — embedded in the binary so users don't need to configure anything.
            // This prevents casual abuse from non-nomadflow traffic but is not a real secret.
            relay_secret: "2990b3a121ae2a13492e71b4e41b33f7d0a7c5beea722974".to_string(),
            subdomain: String::new(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub paths: PathsConfig,
    pub tmux: TmuxConfig,
    pub ttyd: TtydConfig,
    pub api: ApiConfig,
    pub auth: AuthConfig,
    pub tunnel: TunnelConfig,
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

    /// Default config file path (static, always the default location).
    pub fn config_path() -> PathBuf {
        Self::expand_home("~/.nomadflowcode/config.toml")
    }

    /// Check whether the default config file exists on disk.
    pub fn config_exists() -> bool {
        Self::config_path().exists()
    }

    /// Config file path for this instance (relative to base_dir).
    pub fn config_file(&self) -> PathBuf {
        self.base_dir().join("config.toml")
    }

    /// Load settings from the TOML config file.
    pub fn load(config_path: Option<&PathBuf>) -> Result<Self> {
        let path = match config_path {
            Some(p) => p.clone(),
            None => Self::config_path(),
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

    /// Save the current settings to the TOML config file.
    pub fn save(&self) -> Result<()> {
        let path = self.config_file();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| NomadError::Config(format!("Failed to create config dir: {e}")))?;
        }
        let content = toml::to_string_pretty(self)
            .map_err(|e| NomadError::Config(format!("Failed to serialize config: {e}")))?;
        std::fs::write(&path, content)
            .map_err(|e| NomadError::Config(format!("Failed to write config: {e}")))?;
        Ok(())
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
    fn test_save_and_load_round_trip() {
        let tmp = TempDir::new().unwrap();
        let config_path = tmp.path().join("config.toml");

        let settings = Settings {
            paths: PathsConfig {
                base_dir: "/tmp/rt-test".to_string(),
            },
            auth: super::AuthConfig {
                secret: "my-password".to_string(),
            },
            tunnel: super::TunnelConfig {
                subdomain: "my-laptop".to_string(),
                ..Default::default()
            },
            ..Default::default()
        };

        // Save
        let content = toml::to_string_pretty(&settings).unwrap();
        std::fs::write(&config_path, &content).unwrap();

        // Load back
        let loaded = Settings::load(Some(&config_path)).unwrap();
        assert_eq!(loaded.paths.base_dir, "/tmp/rt-test");
        assert_eq!(loaded.auth.secret, "my-password");
        assert_eq!(loaded.tunnel.subdomain, "my-laptop");
        assert_eq!(loaded.api.port, 8080); // default preserved
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
