use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use nomadflow_core::config::Settings;

/// Persisted CLI state.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CliState {
    pub last_server: Option<String>,
    pub last_repo: Option<String>,
    pub last_feature: Option<String>,
    pub last_attached: Option<u64>,
}

/// Server configuration for the TUI (loaded from cli-servers.json).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerConfig {
    pub id: String,
    pub name: String,
    pub api_url: Option<String>,
    pub ttyd_url: Option<String>,
    pub auth_token: Option<String>,
}

/// Derive ttyd URL from API URL (same host, port 7681).
pub fn derive_ttyd_url(api_url: &str) -> String {
    if let Ok(mut url) = url::Url::parse(api_url) {
        url.set_port(Some(7681)).ok();
        url.set_path("/");
        url.to_string().trim_end_matches('/').to_string()
    } else {
        "http://localhost:7681".to_string()
    }
}

fn state_path(settings: &Settings) -> PathBuf {
    settings.base_dir().join("cli-state.json")
}

pub fn load_state(settings: &Settings) -> CliState {
    let path = state_path(settings);
    if path.exists() {
        std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    } else {
        CliState::default()
    }
}

pub fn save_state(settings: &Settings, state: &CliState) {
    let base = settings.base_dir();
    std::fs::create_dir_all(&base).ok();
    let path = state_path(settings);
    if let Ok(json) = serde_json::to_string_pretty(state) {
        std::fs::write(path, json).ok();
    }
}

/// Load server configs: always include localhost, then merge cli-servers.json.
pub fn load_servers(settings: &Settings) -> Vec<ServerConfig> {
    let api_url = format!("http://localhost:{}", settings.api.port);
    let localhost = ServerConfig {
        id: "localhost".to_string(),
        name: "localhost".to_string(),
        ttyd_url: Some(derive_ttyd_url(&api_url)),
        api_url: Some(api_url),
        auth_token: if settings.auth.secret.is_empty() {
            None
        } else {
            Some(settings.auth.secret.clone())
        },
    };

    let servers_path = settings.base_dir().join("cli-servers.json");
    if servers_path.exists() {
        if let Ok(content) = std::fs::read_to_string(&servers_path) {
            if let Ok(mut servers) = serde_json::from_str::<Vec<ServerConfig>>(&content) {
                let has_localhost = servers.iter().any(|s| s.id == "localhost");
                if !has_localhost {
                    servers.insert(0, localhost);
                }
                return servers;
            }
        }
    }

    vec![localhost]
}

/// Save servers to cli-servers.json (filtering out localhost which is auto-generated).
pub fn save_servers(settings: &Settings, servers: &[ServerConfig]) {
    let base = settings.base_dir();
    std::fs::create_dir_all(&base).ok();
    let servers_path = base.join("cli-servers.json");
    let to_save: Vec<&ServerConfig> = servers.iter().filter(|s| s.id != "localhost").collect();
    if let Ok(json) = serde_json::to_string_pretty(&to_save) {
        std::fs::write(servers_path, json).ok();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_save_and_load_state() {
        let tmp = TempDir::new().unwrap();
        let settings = Settings {
            paths: nomadflow_core::config::PathsConfig {
                base_dir: tmp.path().to_string_lossy().to_string(),
            },
            ..Default::default()
        };
        settings.ensure_directories().unwrap();

        let state = CliState {
            last_server: Some("localhost".to_string()),
            last_repo: Some("/tmp/repo".to_string()),
            last_feature: Some("feat".to_string()),
            last_attached: Some(12345),
        };

        save_state(&settings, &state);
        let loaded = load_state(&settings);

        assert_eq!(loaded.last_server.as_deref(), Some("localhost"));
        assert_eq!(loaded.last_repo.as_deref(), Some("/tmp/repo"));
        assert_eq!(loaded.last_feature.as_deref(), Some("feat"));
    }

    #[test]
    fn test_load_nonexistent_state() {
        let tmp = TempDir::new().unwrap();
        let settings = Settings {
            paths: nomadflow_core::config::PathsConfig {
                base_dir: tmp.path().join("nonexistent").to_string_lossy().to_string(),
            },
            ..Default::default()
        };

        let loaded = load_state(&settings);
        assert!(loaded.last_server.is_none());
    }

    #[test]
    fn test_load_corrupted_state() {
        let tmp = TempDir::new().unwrap();
        let settings = Settings {
            paths: nomadflow_core::config::PathsConfig {
                base_dir: tmp.path().to_string_lossy().to_string(),
            },
            ..Default::default()
        };

        // Write invalid JSON
        let path = tmp.path().join("cli-state.json");
        std::fs::write(&path, "not valid json!!!").unwrap();

        let loaded = load_state(&settings);
        assert!(loaded.last_server.is_none()); // Fallback to default
    }
}
