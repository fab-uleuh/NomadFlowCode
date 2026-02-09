use nomadflow_core::models::{
    Feature, ListFeaturesResponse, ListReposResponse, Repository,
};

use crate::state::ServerConfig;

/// Derive the API base URL from a server config.
pub fn get_api_base_url(server: &ServerConfig) -> String {
    if let Some(ref url) = server.api_url {
        let base = url.trim_end_matches('/');
        if base.ends_with("/api") {
            return base.to_string();
        }
        return format!("{base}/api");
    }

    let ttyd_url = server
        .ttyd_url
        .as_deref()
        .unwrap_or("http://localhost:7681");

    // Try to parse and derive port
    if let Ok(url) = url::Url::parse(ttyd_url) {
        let port = if url.port() == Some(7681) {
            8080
        } else {
            url.port().unwrap_or(8080)
        };
        let host = url.host_str().unwrap_or("localhost");
        let scheme = url.scheme();
        return format!("{scheme}://{host}:{port}/api");
    }

    // Fallback
    "http://localhost:8080/api".to_string()
}

/// Check if a server is healthy.
pub async fn check_health(server: &ServerConfig) -> bool {
    let base = get_api_base_url(server).replace("/api", "");
    let url = format!("{base}/health");

    let client = reqwest::Client::new();
    let mut req = client.get(&url).timeout(std::time::Duration::from_secs(3));
    if let Some(ref token) = server.auth_token {
        req = req.header("Authorization", format!("Bearer {token}"));
    }

    req.send().await.map(|r| r.status().is_success()).unwrap_or(false)
}

/// List repos from the server.
pub async fn list_repos(server: &ServerConfig) -> Result<Vec<Repository>, String> {
    let url = format!("{}/list-repos", get_api_base_url(server));

    let client = reqwest::Client::new();
    let mut req = client
        .post(&url)
        .header("Content-Type", "application/json")
        .timeout(std::time::Duration::from_secs(10));

    if let Some(ref token) = server.auth_token {
        req = req.header("Authorization", format!("Bearer {token}"));
    }

    let resp = req.send().await.map_err(|e| e.to_string())?;

    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status()));
    }

    let data: ListReposResponse = resp.json().await.map_err(|e| e.to_string())?;
    Ok(data.repos)
}

/// List features for a repo.
pub async fn list_features(
    server: &ServerConfig,
    repo_path: &str,
) -> Result<Vec<Feature>, String> {
    let url = format!("{}/list-features", get_api_base_url(server));

    let client = reqwest::Client::new();
    let mut req = client
        .post(&url)
        .header("Content-Type", "application/json")
        .json(&serde_json::json!({ "repoPath": repo_path }))
        .timeout(std::time::Duration::from_secs(10));

    if let Some(ref token) = server.auth_token {
        req = req.header("Authorization", format!("Bearer {token}"));
    }

    let resp = req.send().await.map_err(|e| e.to_string())?;

    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status()));
    }

    let data: ListFeaturesResponse = resp.json().await.map_err(|e| e.to_string())?;
    Ok(data.features)
}

/// Create a feature.
pub async fn create_feature(
    server: &ServerConfig,
    repo_path: &str,
    feature_name: &str,
) -> Result<String, String> {
    let url = format!("{}/create-feature", get_api_base_url(server));

    let client = reqwest::Client::new();
    let mut req = client
        .post(&url)
        .header("Content-Type", "application/json")
        .json(&serde_json::json!({
            "repoPath": repo_path,
            "featureName": feature_name,
        }))
        .timeout(std::time::Duration::from_secs(30));

    if let Some(ref token) = server.auth_token {
        req = req.header("Authorization", format!("Bearer {token}"));
    }

    let resp = req.send().await.map_err(|e| e.to_string())?;

    if !resp.status().is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("Failed to create feature: {body}"));
    }

    Ok(feature_name.to_string())
}

/// Switch to a feature (prepares tmux window).
pub async fn switch_feature(
    server: &ServerConfig,
    repo_path: &str,
    feature_name: &str,
) -> Result<String, String> {
    let url = format!("{}/switch-feature", get_api_base_url(server));

    let client = reqwest::Client::new();
    let mut req = client
        .post(&url)
        .header("Content-Type", "application/json")
        .json(&serde_json::json!({
            "repoPath": repo_path,
            "featureName": feature_name,
        }))
        .timeout(std::time::Duration::from_secs(10));

    if let Some(ref token) = server.auth_token {
        req = req.header("Authorization", format!("Bearer {token}"));
    }

    let resp = req.send().await.map_err(|e| e.to_string())?;

    if !resp.status().is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("Failed to switch: {body}"));
    }

    Ok(feature_name.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_api_base_url_with_explicit_api_url() {
        let server = ServerConfig {
            id: "test".to_string(),
            name: "test".to_string(),
            api_url: Some("http://myserver:9000".to_string()),
            ttyd_url: None,
            auth_token: None,
        };
        assert_eq!(get_api_base_url(&server), "http://myserver:9000/api");
    }

    #[test]
    fn test_api_base_url_with_ttyd_port_7681() {
        let server = ServerConfig {
            id: "test".to_string(),
            name: "test".to_string(),
            api_url: None,
            ttyd_url: Some("http://myhost:7681".to_string()),
            auth_token: None,
        };
        assert_eq!(get_api_base_url(&server), "http://myhost:8080/api");
    }

    #[test]
    fn test_api_base_url_fallback() {
        let server = ServerConfig {
            id: "test".to_string(),
            name: "test".to_string(),
            api_url: None,
            ttyd_url: None,
            auth_token: None,
        };
        assert_eq!(get_api_base_url(&server), "http://localhost:8080/api");
    }
}
