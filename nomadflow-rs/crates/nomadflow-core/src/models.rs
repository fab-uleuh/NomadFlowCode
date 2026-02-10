use serde::{Deserialize, Serialize};

// ---- Response models ----

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Repository {
    pub name: String,
    pub path: String,
    pub branch: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Feature {
    pub name: String,
    pub worktree_path: String,
    pub branch: String,
    #[serde(default)]
    pub is_active: bool,
    #[serde(default)]
    pub is_main: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListReposResponse {
    pub repos: Vec<Repository>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListFeaturesResponse {
    pub features: Vec<Feature>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateFeatureResponse {
    pub worktree_path: String,
    pub branch: String,
    pub tmux_window: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteFeatureResponse {
    pub deleted: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SwitchFeatureResponse {
    pub switched: bool,
    pub worktree_path: String,
    pub tmux_window: String,
    #[serde(default)]
    pub has_running_process: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CloneRepoResponse {
    pub name: String,
    pub path: String,
    pub branch: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthResponse {
    pub status: String,
    pub tmux_session: String,
    pub api_port: u16,
}

// ---- Branch models ----

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BranchInfo {
    pub name: String,
    pub is_remote: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remote_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListBranchesRequest {
    pub repo_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListBranchesResponse {
    pub branches: Vec<BranchInfo>,
    pub default_branch: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AttachBranchRequest {
    pub repo_path: String,
    pub branch_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AttachBranchResponse {
    pub worktree_path: String,
    pub branch: String,
    pub tmux_window: String,
}

// ---- Request models ----

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListFeaturesRequest {
    pub repo_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateFeatureRequest {
    pub repo_path: String,
    /// Full branch name (e.g. "feature/add-login", "bugfix/crash", "my-branch")
    #[serde(alias = "featureName")]
    pub branch_name: String,
    #[serde(default = "default_base_branch")]
    pub base_branch: String,
}

fn default_base_branch() -> String {
    "main".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteFeatureRequest {
    pub repo_path: String,
    pub feature_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SwitchFeatureRequest {
    pub repo_path: String,
    pub feature_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CloneRepoRequest {
    pub url: String,
    pub token: Option<String>,
    pub name: Option<String>,
}

// ---- Server model (for TUI config) ----

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Server {
    pub id: String,
    pub name: String,
    pub api_url: Option<String>,
    pub auth_token: Option<String>,
    pub last_connected: Option<u64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_feature_serialization_camel_case() {
        let feature = Feature {
            name: "my-feature".to_string(),
            worktree_path: "/tmp/wt".to_string(),
            branch: "feature/my-feature".to_string(),
            is_active: true,
            is_main: false,
        };
        let json = serde_json::to_string(&feature).unwrap();
        assert!(json.contains("\"worktreePath\""));
        assert!(json.contains("\"isActive\""));
        assert!(json.contains("\"isMain\""));
        assert!(!json.contains("\"worktree_path\""));
    }

    #[test]
    fn test_switch_response_serialization() {
        let resp = SwitchFeatureResponse {
            switched: true,
            worktree_path: "/tmp/wt".to_string(),
            tmux_window: "repo:feat".to_string(),
            has_running_process: true,
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("\"hasRunningProcess\""));
        assert!(json.contains("\"tmuxWindow\""));
    }

    #[test]
    fn test_list_features_request_deserialization() {
        let json = r#"{"repoPath": "/tmp/repo"}"#;
        let req: ListFeaturesRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.repo_path, "/tmp/repo");
    }

    #[test]
    fn test_create_feature_request_deserialization() {
        // New format with branchName
        let json = r#"{"branchName": "feature/x", "repoPath": "y", "baseBranch": "main"}"#;
        let req: CreateFeatureRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.branch_name, "feature/x");
        assert_eq!(req.repo_path, "y");
        assert_eq!(req.base_branch, "main");

        // Backward compat with featureName alias
        let json2 = r#"{"featureName": "x", "repoPath": "y", "baseBranch": "main"}"#;
        let req2: CreateFeatureRequest = serde_json::from_str(json2).unwrap();
        assert_eq!(req2.branch_name, "x");
    }

    #[test]
    fn test_round_trip_feature() {
        let original = Feature {
            name: "test".to_string(),
            worktree_path: "/a/b".to_string(),
            branch: "feature/test".to_string(),
            is_active: false,
            is_main: true,
        };
        let json = serde_json::to_string(&original).unwrap();
        let deserialized: Feature = serde_json::from_str(&json).unwrap();
        assert_eq!(original.name, deserialized.name);
        assert_eq!(original.worktree_path, deserialized.worktree_path);
        assert_eq!(original.is_main, deserialized.is_main);
    }
}
