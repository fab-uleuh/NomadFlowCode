use serde::{Deserialize, Serialize};

// ---- Worktree detection ----

#[derive(Debug, Clone)]
pub struct WorktreeInfo {
    pub repo_name: String,
    pub repo_path: String,
    pub worktree_name: String,
    pub worktree_path: String,
}

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
    pub worktree_name: String,
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
    pub worktree_name: String,
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
    pub worktree_name: String,
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
    pub linked_session: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CloneRepoRequest {
    pub url: String,
    pub token: Option<String>,
    pub name: Option<String>,
}

// ---- Session models ----

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Session {
    pub session_id: String,
    pub window_name: String,
    pub repo: String,
    pub worktree: String,
    pub agent_type: String,
    pub agent_number: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionWithState {
    pub session_id: String,
    pub window_name: String,
    pub repo: String,
    pub worktree: String,
    pub agent_type: String,
    pub agent_number: u32,
    pub agent_state: AgentStateKind,
    pub state_timestamp: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListSessionsResponse {
    pub sessions: Vec<SessionWithState>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateSessionRequest {
    pub worktree_path: String,
    pub agent_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateSessionResponse {
    pub session: Session,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CloseSessionRequest {
    pub session_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CloseSessionResponse {
    pub closed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectSessionRequest {
    pub session_id: String,
    /// Optional linked session name (e.g. "nomadflow-0") for independent cursor targeting.
    /// When provided, select-window targets this linked session instead of the base session.
    pub linked_session: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectSessionResponse {
    pub selected: bool,
}

// ---- Agent state models ----

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentStateKind {
    WaitingForInput,
    WaitingForPermission,
    Generating,
    Idle,
    Done,
    Error,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentState {
    pub state: AgentStateKind,
    pub timestamp: String,
    pub last_event: String,
}

// ---- Git diff/status models ----

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorktreeStatusRequest {
    pub worktree_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorktreeStatusResponse {
    pub worktree_path: String,
    pub branch: String,
    pub files: Vec<FileChange>,
    pub summary: StatusSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileChange {
    pub path: String,
    pub status: String,
    pub additions: usize,
    pub deletions: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusSummary {
    pub modified: usize,
    pub new: usize,
    pub deleted: usize,
    pub conflicted: usize,
    pub total_additions: usize,
    pub total_deletions: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileDiffRequest {
    pub worktree_path: String,
    pub file_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileDiffResponse {
    pub file_path: String,
    pub hunks: Vec<DiffHunk>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiffHunk {
    pub old_start: u32,
    pub old_lines: u32,
    pub new_start: u32,
    pub new_lines: u32,
    pub lines: Vec<DiffLine>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiffLine {
    #[serde(rename = "type")]
    pub line_type: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileContentRequest {
    pub worktree_path: String,
    pub file_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileContentResponse {
    pub file_path: String,
    pub content: String,
}

// ---- File tree models ----

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListDirRequest {
    pub worktree_path: String,
    /// Relative path within the worktree (empty string or "." for root)
    pub relative_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DirEntry {
    pub name: String,
    /// Relative path from worktree root (e.g. "src/components")
    pub path: String,
    pub is_dir: bool,
    /// File size in bytes (0 for directories)
    pub size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListDirResponse {
    pub entries: Vec<DirEntry>,
    /// The path that was listed (echoed back for client verification)
    pub path: String,
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
            worktree_name: "repo:feat".to_string(),
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("\"worktreeName\""));
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
    fn test_session_serialization_camel_case() {
        let session = Session {
            session_id: "myapp-feat-auth-agent-1".to_string(),
            window_name: "myapp:feat-auth:agent-1".to_string(),
            repo: "myapp".to_string(),
            worktree: "feat-auth".to_string(),
            agent_type: "agent".to_string(),
            agent_number: 1,
        };
        let json = serde_json::to_string(&session).unwrap();
        assert!(json.contains("\"sessionId\""));
        assert!(json.contains("\"windowName\""));
        assert!(json.contains("\"agentType\""));
        assert!(json.contains("\"agentNumber\""));
        // Must NOT contain snake_case
        assert!(!json.contains("\"session_id\""));
        assert!(!json.contains("\"window_name\""));
        assert!(!json.contains("\"agent_type\""));
        assert!(!json.contains("\"agent_number\""));
    }

    #[test]
    fn test_create_session_request_deserialization() {
        let json = r#"{"worktreePath": "/tmp/wt", "agentType": "claude"}"#;
        let req: CreateSessionRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.worktree_path, "/tmp/wt");
        assert_eq!(req.agent_type, Some("claude".to_string()));

        // agentType is optional
        let json2 = r#"{"worktreePath": "/tmp/wt"}"#;
        let req2: CreateSessionRequest = serde_json::from_str(json2).unwrap();
        assert_eq!(req2.agent_type, None);
    }

    #[test]
    fn test_close_session_request_deserialization() {
        let json = r#"{"sessionId": "myapp-feat-agent-1"}"#;
        let req: CloseSessionRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.session_id, "myapp-feat-agent-1");
    }

    #[test]
    fn test_session_with_state_serialization_camel_case() {
        let session = SessionWithState {
            session_id: "myapp-feat-auth-claude-code-1".to_string(),
            window_name: "myapp:feat-auth:claude-code-1".to_string(),
            repo: "myapp".to_string(),
            worktree: "feat-auth".to_string(),
            agent_type: "claude-code".to_string(),
            agent_number: 1,
            agent_state: AgentStateKind::WaitingForInput,
            state_timestamp: Some("2026-02-15T14:30:00Z".to_string()),
        };
        let json = serde_json::to_string(&session).unwrap();
        // camelCase field names
        assert!(json.contains("\"sessionId\""));
        assert!(json.contains("\"windowName\""));
        assert!(json.contains("\"agentType\""));
        assert!(json.contains("\"agentNumber\""));
        assert!(json.contains("\"agentState\""));
        assert!(json.contains("\"stateTimestamp\""));
        // snake_case state values
        assert!(json.contains("\"waiting_for_input\""));
        // Must NOT contain snake_case field names
        assert!(!json.contains("\"session_id\""));
        assert!(!json.contains("\"agent_state\""));
        assert!(!json.contains("\"state_timestamp\""));
    }

    #[test]
    fn test_session_with_state_deserialization() {
        let json = r#"{"sessionId":"myapp-feat-claude-code-1","windowName":"myapp:feat:claude-code-1","repo":"myapp","worktree":"feat","agentType":"claude-code","agentNumber":1,"agentState":"generating","stateTimestamp":"2026-02-15T14:30:00Z"}"#;
        let session: SessionWithState = serde_json::from_str(json).unwrap();
        assert_eq!(session.session_id, "myapp-feat-claude-code-1");
        assert_eq!(session.agent_state, AgentStateKind::Generating);
        assert_eq!(
            session.state_timestamp,
            Some("2026-02-15T14:30:00Z".to_string())
        );
    }

    #[test]
    fn test_session_with_state_unknown_no_timestamp() {
        let session = SessionWithState {
            session_id: "myapp-feat-generic-1".to_string(),
            window_name: "myapp:feat:generic-1".to_string(),
            repo: "myapp".to_string(),
            worktree: "feat".to_string(),
            agent_type: "generic".to_string(),
            agent_number: 1,
            agent_state: AgentStateKind::Unknown,
            state_timestamp: None,
        };
        let json = serde_json::to_string(&session).unwrap();
        assert!(json.contains("\"agentState\":\"unknown\""));
        // stateTimestamp should be null (None → null in JSON)
        assert!(json.contains("\"stateTimestamp\":null"));
    }

    #[test]
    fn test_agent_state_kind_unknown_serialization() {
        let kind = AgentStateKind::Unknown;
        let json = serde_json::to_string(&kind).unwrap();
        assert_eq!(json, "\"unknown\"");

        let deserialized: AgentStateKind = serde_json::from_str("\"unknown\"").unwrap();
        assert_eq!(deserialized, AgentStateKind::Unknown);
    }

    #[test]
    fn test_list_sessions_response_with_enriched_sessions() {
        let response = ListSessionsResponse {
            sessions: vec![
                SessionWithState {
                    session_id: "app-feat-claude-code-1".to_string(),
                    window_name: "app:feat:claude-code-1".to_string(),
                    repo: "app".to_string(),
                    worktree: "feat".to_string(),
                    agent_type: "claude-code".to_string(),
                    agent_number: 1,
                    agent_state: AgentStateKind::WaitingForInput,
                    state_timestamp: Some("2026-02-15T14:30:00Z".to_string()),
                },
                SessionWithState {
                    session_id: "app-feat-generic-1".to_string(),
                    window_name: "app:feat:generic-1".to_string(),
                    repo: "app".to_string(),
                    worktree: "feat".to_string(),
                    agent_type: "generic".to_string(),
                    agent_number: 1,
                    agent_state: AgentStateKind::Unknown,
                    state_timestamp: None,
                },
            ],
        };
        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"sessions\""));
        assert!(json.contains("\"waiting_for_input\""));
        assert!(json.contains("\"unknown\""));
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
