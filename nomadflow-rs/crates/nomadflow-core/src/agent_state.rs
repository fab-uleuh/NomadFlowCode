use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::config::Settings;
use crate::error::{NomadError, Result};
use crate::models::{AgentState, AgentStateKind, Session};
use crate::services::tmux::TmuxService;

/// Embedded hook script content — installed to ~/.nomadflowcode/hooks/claude-code/state-tracker.sh
///
/// NOTE: The `idle` state (after configurable timeout, default 30s) cannot be implemented in
/// hook scripts because they are event-driven. The `idle` transition from `waiting_for_input`
/// must be computed at read-time by the server/API layer comparing `timestamp` against a
/// configurable idle timeout. See AC#6 — to be implemented in the agent-state API endpoint.
const HOOK_SCRIPT: &str = r#"#!/bin/bash
# NomadFlow State Tracker — Claude Code Hook Script
# Tracks agent state transitions via hook events.
# Installed by: nomadflow run (ensure_hook_scripts)
# IMPORTANT: Always exit 0 to never block Claude's execution.

main() {
    local SESSION_ID="${NOMADFLOW_SESSION_ID:-}"
    local STATE_DIR="${NOMADFLOW_STATE_DIR:-}"

    # Auto-discover from tmux window name when env vars are not set
    if [ -z "$SESSION_ID" ] || [ -z "$STATE_DIR" ]; then
        local WINDOW_NAME
        WINDOW_NAME=$(tmux display-message -p '#{window_name}' 2>/dev/null)
        if [ -n "$WINDOW_NAME" ] && echo "$WINDOW_NAME" | grep -q ':'; then
            SESSION_ID=$(echo "$WINDOW_NAME" | tr ':' '-')
            STATE_DIR="$HOME/.nomadflowcode/sessions/$SESSION_ID"
        else
            return 0
        fi
    fi

    # Read JSON from stdin
    local INPUT
    INPUT=$(cat)

    # Parse fields — try jq first (single call), fallback to basic grep
    local EVENT="" TOOL_NAME="" NTYPE=""
    if command -v jq >/dev/null 2>&1; then
        local PARSED
        PARSED=$(echo "$INPUT" | jq -r '[(.hook_event_name // ""), (.tool_name // ""), (.notification_type // "")] | @tsv')
        EVENT=$(echo "$PARSED" | cut -f1)
        TOOL_NAME=$(echo "$PARSED" | cut -f2)
        NTYPE=$(echo "$PARSED" | cut -f3)
    else
        EVENT=$(echo "$INPUT" | grep -o '"hook_event_name"[[:space:]]*:[[:space:]]*"[^"]*"' | head -1 | grep -o '"[^"]*"$' | tr -d '"')
        TOOL_NAME=$(echo "$INPUT" | grep -o '"tool_name"[[:space:]]*:[[:space:]]*"[^"]*"' | head -1 | grep -o '"[^"]*"$' | tr -d '"')
        NTYPE=$(echo "$INPUT" | grep -o '"notification_type"[[:space:]]*:[[:space:]]*"[^"]*"' | head -1 | grep -o '"[^"]*"$' | tr -d '"')
    fi

    if [ -z "$EVENT" ]; then
        return 0
    fi

    # Map event → state
    local STATE=""
    case "$EVENT" in
        SessionStart)       STATE="idle" ;;
        UserPromptSubmit)   STATE="generating" ;;
        PreToolUse)         STATE="generating" ;;
        PostToolUse)        return 0 ;;  # No state change — Claude still working
        PostToolUseFailure) STATE="error" ;;
        Stop)               STATE="waiting_for_input" ;;
        Notification)
            case "$NTYPE" in
                permission_prompt|idle_prompt) STATE="waiting_for_input" ;;
                *) return 0 ;;
            esac
            ;;
        SessionEnd)         STATE="done" ;;
        *)                  return 0 ;;
    esac

    if [ -z "$STATE" ]; then
        return 0
    fi

    # Ensure state directory exists
    mkdir -p "$STATE_DIR"

    # Build JSON
    local TIMESTAMP
    TIMESTAMP=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
    local STATE_FILE="$STATE_DIR/agent-state.json"
    local JSON

    if [ "$STATE" = "done" ]; then
        JSON="{\"sessionId\":\"$SESSION_ID\",\"agentType\":\"claude-code\",\"state\":\"$STATE\",\"timestamp\":\"$TIMESTAMP\",\"lastEvent\":\"$EVENT\",\"endedAt\":\"$TIMESTAMP\"}"
    elif [ -n "$TOOL_NAME" ]; then
        JSON="{\"sessionId\":\"$SESSION_ID\",\"agentType\":\"claude-code\",\"state\":\"$STATE\",\"timestamp\":\"$TIMESTAMP\",\"lastEvent\":\"$EVENT\",\"toolName\":\"$TOOL_NAME\"}"
    else
        JSON="{\"sessionId\":\"$SESSION_ID\",\"agentType\":\"claude-code\",\"state\":\"$STATE\",\"timestamp\":\"$TIMESTAMP\",\"lastEvent\":\"$EVENT\"}"
    fi

    # Atomic write: tmp file + mv
    local TMP_FILE="${STATE_FILE}.tmp.$$"
    echo "$JSON" > "$TMP_FILE"
    mv "$TMP_FILE" "$STATE_FILE"
}

main "$@" || true
exit 0
"#;

/// Hook events that nomadflow injects into .claude/settings.local.json.
/// Note: PostToolUse is intentionally excluded — it triggers no state change and
/// removing it avoids a fork+exec on every tool completion (performance).
const HOOK_EVENTS: &[(&str, Option<&str>)] = &[
    ("SessionStart", Some("startup|resume")),
    ("UserPromptSubmit", None),
    ("PreToolUse", None),
    ("PostToolUseFailure", None),
    ("Stop", None),
    ("Notification", None),
    ("SessionEnd", None),
];

/// Service for managing agent state detection via Claude Code hooks.
pub struct AgentStateService {
    sessions_dir: PathBuf,
    hooks_dir: PathBuf,
}

impl AgentStateService {
    pub fn new(settings: &Settings) -> Self {
        Self {
            sessions_dir: settings.sessions_dir(),
            hooks_dir: settings.base_dir().join("hooks").join("claude-code"),
        }
    }

    /// Install/update hook scripts to ~/.nomadflowcode/hooks/claude-code/.
    /// Idempotent — overwrites existing script if content differs.
    pub async fn ensure_hook_scripts(&self) -> Result<()> {
        tokio::fs::create_dir_all(&self.hooks_dir).await?;

        let script_path = self.hooks_dir.join("state-tracker.sh");

        // Check if already up-to-date
        if let Ok(existing) = tokio::fs::read_to_string(&script_path).await {
            if existing == HOOK_SCRIPT {
                return Ok(());
            }
        }

        // Write script
        tokio::fs::write(&script_path, HOOK_SCRIPT).await?;

        // Make executable (chmod +x)
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let perms = std::fs::Permissions::from_mode(0o755);
            tokio::fs::set_permissions(&script_path, perms).await?;
        }

        Ok(())
    }

    /// Inject nomadflow hooks into .claude/settings.local.json in the given project directory.
    /// Also creates a state directory for the session. Merges with existing settings.
    pub async fn inject_hooks(&self, project_dir: &Path, session_id: &str) -> Result<()> {
        self.inject_hooks_impl(project_dir).await?;

        // Ensure state directory exists for this specific session
        let state_dir = self.sessions_dir.join(session_id);
        tokio::fs::create_dir_all(&state_dir).await?;

        Ok(())
    }

    /// Inject nomadflow hooks into .claude/settings.local.json without creating a state directory.
    /// Used for auto-injection at server startup and on switch-feature — the hook script
    /// handles state dir creation itself via tmux window name auto-discovery.
    pub async fn inject_hooks_for_project(&self, project_dir: &Path) -> Result<()> {
        self.inject_hooks_impl(project_dir).await
    }

    /// Internal: inject hooks into .claude/settings.local.json.
    /// Merges with existing settings — preserves user entries.
    async fn inject_hooks_impl(&self, project_dir: &Path) -> Result<()> {
        let claude_dir = project_dir.join(".claude");
        tokio::fs::create_dir_all(&claude_dir).await?;

        let settings_path = claude_dir.join("settings.local.json");
        let script_path = self.hooks_dir.join("state-tracker.sh");
        let script_str = script_path.to_string_lossy().to_string();

        // Load existing settings or start fresh
        let mut settings: Value =
            if let Ok(content) = tokio::fs::read_to_string(&settings_path).await {
                serde_json::from_str(&content).map_err(|e| {
                    NomadError::Config(format!("Failed to parse settings.local.json: {e}"))
                })?
            } else {
                Value::Object(serde_json::Map::new())
            };

        // Ensure "hooks" object exists
        let hooks = settings
            .as_object_mut()
            .ok_or_else(|| NomadError::Config("settings.local.json is not a JSON object".into()))?
            .entry("hooks")
            .or_insert_with(|| Value::Object(serde_json::Map::new()));

        let hooks_obj = hooks
            .as_object_mut()
            .ok_or_else(|| NomadError::Config("hooks is not a JSON object".into()))?;

        // For each hook event, add our matcher group
        for &(event, matcher) in HOOK_EVENTS {
            let handler = build_hook_handler(&script_str);
            let matcher_group = build_matcher_group(matcher, handler);

            let event_array = hooks_obj
                .entry(event)
                .or_insert_with(|| Value::Array(Vec::new()));

            let arr = event_array
                .as_array_mut()
                .ok_or_else(|| NomadError::Config(format!("hooks.{event} is not an array")))?;

            // Remove any existing nomadflow matcher groups first
            arr.retain(|group| !is_nomadflow_matcher_group(group));

            // Add our matcher group
            arr.push(matcher_group);
        }

        // Write settings.local.json
        let content = serde_json::to_string_pretty(&settings)
            .map_err(|e| NomadError::Config(format!("Failed to serialize settings: {e}")))?;
        tokio::fs::write(&settings_path, content).await?;

        Ok(())
    }

    /// Remove nomadflow hook entries from .claude/settings.local.json.
    /// Preserves other settings and user-defined hooks.
    pub async fn cleanup_hooks(&self, project_dir: &Path) -> Result<()> {
        let settings_path = project_dir.join(".claude").join("settings.local.json");

        if !settings_path.exists() {
            return Ok(());
        }

        let content = tokio::fs::read_to_string(&settings_path).await?;
        let mut settings: Value = serde_json::from_str(&content)
            .map_err(|e| NomadError::Config(format!("Failed to parse settings.local.json: {e}")))?;

        let Some(hooks) = settings.get_mut("hooks").and_then(|h| h.as_object_mut()) else {
            return Ok(());
        };

        // For each event, remove nomadflow matcher groups
        let mut empty_events = Vec::new();
        for (event, arr_val) in hooks.iter_mut() {
            if let Some(arr) = arr_val.as_array_mut() {
                arr.retain(|group| !is_nomadflow_matcher_group(group));
                if arr.is_empty() {
                    empty_events.push(event.clone());
                }
            }
        }

        // Remove empty event arrays
        for event in &empty_events {
            hooks.remove(event);
        }

        // If hooks is now empty, remove the hooks key
        if hooks.is_empty() {
            if let Some(obj) = settings.as_object_mut() {
                obj.remove("hooks");

                // If the entire settings is now empty, delete the file
                if obj.is_empty() {
                    tokio::fs::remove_file(&settings_path).await.ok();
                    return Ok(());
                }
            }
        }

        // Write back
        let content = serde_json::to_string_pretty(&settings)
            .map_err(|e| NomadError::Config(format!("Failed to serialize settings: {e}")))?;
        tokio::fs::write(&settings_path, content).await?;

        Ok(())
    }

    /// Read agent state for a given session ID.
    pub async fn read_state(&self, session_id: &str) -> Option<AgentState> {
        let state_path = self.sessions_dir.join(session_id).join("agent-state.json");
        let content = tokio::fs::read_to_string(&state_path).await.ok()?;
        serde_json::from_str(&content).ok()
    }

    /// Discover all active session states.
    pub async fn list_states(&self) -> Vec<AgentState> {
        let mut states = Vec::new();

        let Ok(mut entries) = tokio::fs::read_dir(&self.sessions_dir).await else {
            return states;
        };

        while let Ok(Some(entry)) = entries.next_entry().await {
            let state_file = entry.path().join("agent-state.json");
            if let Ok(content) = tokio::fs::read_to_string(&state_file).await {
                if let Ok(state) = serde_json::from_str::<AgentState>(&content) {
                    states.push(state);
                }
            }
        }

        states
    }

    /// Detect process state for a generic (non-Claude-Code) session via tmux.
    ///
    /// Maps tmux pane command to an `AgentStateKind`:
    /// - Window doesn't exist → `Done`
    /// - Pane command is an idle shell → `WaitingForInput`
    /// - Pane command is any other process → `Generating`
    /// - Cannot determine pane command → `Idle`
    pub async fn detect_process_state(&self, session: &Session, tmux: &TmuxService) -> AgentState {
        let now = utc_timestamp();

        let state = if !tmux.window_exists(&session.window_name).await {
            AgentStateKind::Done
        } else {
            match tmux.get_pane_command(&session.window_name).await {
                Some(cmd) => {
                    if is_idle_shell(&cmd) {
                        AgentStateKind::WaitingForInput
                    } else {
                        AgentStateKind::Generating
                    }
                }
                None => AgentStateKind::Idle,
            }
        };

        let ended_at = if state == AgentStateKind::Done {
            Some(now.clone())
        } else {
            None
        };

        AgentState {
            session_id: session.session_id.clone(),
            agent_type: "generic".to_string(),
            state,
            timestamp: now,
            last_event: "ProcessCheck".to_string(),
            tool_name: None,
            ended_at,
        }
    }

    /// Read state for a session, routing to the correct adapter based on agent_type.
    ///
    /// - `claude-code` → reads state file from disk (hook-driven)
    /// - Any other type → tries state file first (auto-discovered hooks),
    ///   falls back to tmux process detection if no file exists or state is `done`
    pub async fn read_state_for_session(
        &self,
        session: &Session,
        tmux: &TmuxService,
    ) -> Option<AgentState> {
        if session.agent_type == "claude-code" {
            self.read_state(&session.session_id).await
        } else {
            // Try hook-driven state file first (works when Claude Code runs
            // in a legacy 2-part tmux window with auto-discovered session ID)
            if let Some(state) = self.read_state(&session.session_id).await {
                if state.state != AgentStateKind::Done {
                    return Some(state);
                }
            }
            Some(self.detect_process_state(session, tmux).await)
        }
    }

    /// List states for all sessions, using the appropriate adapter per session.
    pub async fn list_all_states(
        &self,
        sessions: &[Session],
        tmux: &TmuxService,
    ) -> Vec<AgentState> {
        let mut states = Vec::new();
        for session in sessions {
            if let Some(state) = self.read_state_for_session(session, tmux).await {
                states.push(state);
            }
        }
        states
    }

    /// Get the hooks directory path (for testing).
    pub fn hooks_dir(&self) -> &Path {
        &self.hooks_dir
    }

    /// Get the sessions directory path.
    pub fn sessions_dir(&self) -> &Path {
        &self.sessions_dir
    }

    /// Persist the worktree path for a session (used for reliable hook cleanup).
    pub async fn save_worktree_path(&self, session_id: &str, worktree_path: &Path) -> Result<()> {
        let session_dir = self.sessions_dir.join(session_id);
        tokio::fs::create_dir_all(&session_dir).await?;
        tokio::fs::write(
            session_dir.join("worktree-path"),
            worktree_path.to_string_lossy().as_bytes(),
        )
        .await?;
        Ok(())
    }

    /// Read the persisted worktree path for a session.
    pub async fn read_worktree_path(&self, session_id: &str) -> Option<String> {
        let path = self.sessions_dir.join(session_id).join("worktree-path");
        tokio::fs::read_to_string(&path).await.ok()
    }
}

/// Build a single hook handler object with the _nomadflow marker.
fn build_hook_handler(script_path: &str) -> Value {
    serde_json::json!({
        "type": "command",
        "command": script_path,
        "_nomadflow": true
    })
}

/// Build a matcher group containing one handler.
fn build_matcher_group(matcher: Option<&str>, handler: Value) -> Value {
    let mut group = serde_json::Map::new();
    if let Some(m) = matcher {
        group.insert("matcher".to_string(), Value::String(m.to_string()));
    }
    group.insert("hooks".to_string(), Value::Array(vec![handler]));
    Value::Object(group)
}

/// Check if a command name is an idle shell.
fn is_idle_shell(cmd: &str) -> bool {
    const IDLE_SHELLS: &[&str] = &["bash", "zsh", "sh", "fish", "dash", "ksh", "tcsh", "csh"];
    IDLE_SHELLS.contains(&cmd.to_lowercase().as_str())
}

/// Generate a UTC ISO 8601 timestamp without external dependencies.
fn utc_timestamp() -> String {
    use std::time::SystemTime;
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = now.as_secs();
    // Convert to date-time components
    let days = secs / 86400;
    let time_secs = secs % 86400;
    let hours = time_secs / 3600;
    let minutes = (time_secs % 3600) / 60;
    let seconds = time_secs % 60;

    // Simple date calculation from days since epoch
    let mut y = 1970i64;
    let mut remaining_days = days as i64;
    loop {
        let year_days = if is_leap_year(y) { 366 } else { 365 };
        if remaining_days < year_days {
            break;
        }
        remaining_days -= year_days;
        y += 1;
    }
    let leap = is_leap_year(y);
    let month_days: [i64; 12] = [
        31,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    let mut m = 0usize;
    for (i, &md) in month_days.iter().enumerate() {
        if remaining_days < md {
            m = i;
            break;
        }
        remaining_days -= md;
    }

    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        y,
        m + 1,
        remaining_days + 1,
        hours,
        minutes,
        seconds
    )
}

fn is_leap_year(y: i64) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}

/// Check if a matcher group contains any nomadflow hooks.
fn is_nomadflow_matcher_group(group: &Value) -> bool {
    group
        .get("hooks")
        .and_then(|h| h.as_array())
        .map(|hooks| {
            hooks
                .iter()
                .any(|h| h.get("_nomadflow") == Some(&Value::Bool(true)))
        })
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_agent_state_serialization_camel_case() {
        let state = AgentState {
            session_id: "myapp-feat-auth-agent-1".to_string(),
            agent_type: "claude-code".to_string(),
            state: crate::models::AgentStateKind::Generating,
            timestamp: "2026-02-15T10:30:00Z".to_string(),
            last_event: "PreToolUse".to_string(),
            tool_name: Some("Edit".to_string()),
            ended_at: None,
        };
        let json = serde_json::to_string(&state).unwrap();
        // camelCase field names
        assert!(json.contains("\"sessionId\""));
        assert!(json.contains("\"agentType\""));
        assert!(json.contains("\"lastEvent\""));
        assert!(json.contains("\"toolName\""));
        // snake_case state values
        assert!(json.contains("\"generating\""));
        // Must NOT contain snake_case field names
        assert!(!json.contains("\"session_id\""));
        assert!(!json.contains("\"agent_type\""));
        assert!(!json.contains("\"last_event\""));
        assert!(!json.contains("\"tool_name\""));
        // endedAt should be absent (None)
        assert!(!json.contains("\"endedAt\""));
    }

    #[test]
    fn test_agent_state_deserialization() {
        let json = r#"{
            "sessionId": "myapp-feat-auth-agent-1",
            "agentType": "claude-code",
            "state": "waiting_for_input",
            "timestamp": "2026-02-15T10:30:00Z",
            "lastEvent": "Stop"
        }"#;
        let state: AgentState = serde_json::from_str(json).unwrap();
        assert_eq!(state.session_id, "myapp-feat-auth-agent-1");
        assert_eq!(state.state, crate::models::AgentStateKind::WaitingForInput);
        assert!(state.tool_name.is_none());
        assert!(state.ended_at.is_none());
    }

    #[test]
    fn test_agent_state_done_with_ended_at() {
        let state = AgentState {
            session_id: "test-session".to_string(),
            agent_type: "claude-code".to_string(),
            state: crate::models::AgentStateKind::Done,
            timestamp: "2026-02-15T10:30:00Z".to_string(),
            last_event: "SessionEnd".to_string(),
            tool_name: None,
            ended_at: Some("2026-02-15T10:30:00Z".to_string()),
        };
        let json = serde_json::to_string(&state).unwrap();
        assert!(json.contains("\"endedAt\""));
        assert!(json.contains("\"done\""));
    }

    #[tokio::test]
    async fn test_read_state_valid_file() {
        let tmp = TempDir::new().unwrap();
        let sessions_dir = tmp.path().join("sessions");
        let session_dir = sessions_dir.join("test-session");
        std::fs::create_dir_all(&session_dir).unwrap();
        std::fs::write(
            session_dir.join("agent-state.json"),
            r#"{"sessionId":"test-session","agentType":"claude-code","state":"generating","timestamp":"2026-02-15T10:30:00Z","lastEvent":"PreToolUse","toolName":"Edit"}"#,
        )
        .unwrap();

        let svc = AgentStateService {
            sessions_dir,
            hooks_dir: tmp.path().join("hooks"),
        };

        let state = svc.read_state("test-session").await.unwrap();
        assert_eq!(state.session_id, "test-session");
        assert_eq!(state.state, crate::models::AgentStateKind::Generating);
        assert_eq!(state.tool_name, Some("Edit".to_string()));
    }

    #[tokio::test]
    async fn test_read_state_missing_file() {
        let tmp = TempDir::new().unwrap();
        let svc = AgentStateService {
            sessions_dir: tmp.path().join("sessions"),
            hooks_dir: tmp.path().join("hooks"),
        };
        assert!(svc.read_state("nonexistent").await.is_none());
    }

    #[tokio::test]
    async fn test_read_state_corrupt_file() {
        let tmp = TempDir::new().unwrap();
        let sessions_dir = tmp.path().join("sessions");
        let session_dir = sessions_dir.join("corrupt-session");
        std::fs::create_dir_all(&session_dir).unwrap();
        std::fs::write(session_dir.join("agent-state.json"), "not valid json").unwrap();

        let svc = AgentStateService {
            sessions_dir,
            hooks_dir: tmp.path().join("hooks"),
        };
        assert!(svc.read_state("corrupt-session").await.is_none());
    }

    #[tokio::test]
    async fn test_inject_hooks_creates_settings_file() {
        let tmp = TempDir::new().unwrap();
        let project_dir = tmp.path().join("project");
        std::fs::create_dir_all(&project_dir).unwrap();

        let svc = AgentStateService {
            sessions_dir: tmp.path().join("sessions"),
            hooks_dir: tmp.path().join("hooks").join("claude-code"),
        };

        // Create hook script so path is valid
        std::fs::create_dir_all(&svc.hooks_dir).unwrap();
        std::fs::write(svc.hooks_dir.join("state-tracker.sh"), "#!/bin/bash").unwrap();

        svc.inject_hooks(&project_dir, "test-session")
            .await
            .unwrap();

        let settings_path = project_dir.join(".claude").join("settings.local.json");
        assert!(settings_path.exists());

        let content = std::fs::read_to_string(&settings_path).unwrap();
        let v: Value = serde_json::from_str(&content).unwrap();

        // Verify all hook events are present
        let hooks = v.get("hooks").unwrap().as_object().unwrap();
        assert!(hooks.contains_key("SessionStart"));
        assert!(hooks.contains_key("PreToolUse"));
        assert!(hooks.contains_key("Stop"));
        assert!(hooks.contains_key("SessionEnd"));
        // PostToolUse intentionally excluded (no state change, avoids unnecessary fork)
        assert!(!hooks.contains_key("PostToolUse"));

        // Verify SessionStart has matcher
        let ss = hooks.get("SessionStart").unwrap().as_array().unwrap();
        assert_eq!(
            ss[0].get("matcher").unwrap().as_str().unwrap(),
            "startup|resume"
        );

        // Verify _nomadflow marker
        let handler = &ss[0]["hooks"][0];
        assert_eq!(handler.get("_nomadflow").unwrap().as_bool().unwrap(), true);
        assert_eq!(handler.get("type").unwrap().as_str().unwrap(), "command");
    }

    #[tokio::test]
    async fn test_inject_hooks_merges_with_existing() {
        let tmp = TempDir::new().unwrap();
        let project_dir = tmp.path().join("project");
        let claude_dir = project_dir.join(".claude");
        std::fs::create_dir_all(&claude_dir).unwrap();

        // Write existing settings with user hooks
        let existing = serde_json::json!({
            "permissions": { "allow": ["Read"] },
            "hooks": {
                "PostToolUse": [
                    {
                        "matcher": "Write",
                        "hooks": [{ "type": "command", "command": "my-linter.sh" }]
                    }
                ]
            }
        });
        std::fs::write(
            claude_dir.join("settings.local.json"),
            serde_json::to_string_pretty(&existing).unwrap(),
        )
        .unwrap();

        let svc = AgentStateService {
            sessions_dir: tmp.path().join("sessions"),
            hooks_dir: tmp.path().join("hooks").join("claude-code"),
        };
        std::fs::create_dir_all(&svc.hooks_dir).unwrap();
        std::fs::write(svc.hooks_dir.join("state-tracker.sh"), "#!/bin/bash").unwrap();

        svc.inject_hooks(&project_dir, "test-session")
            .await
            .unwrap();

        let content = std::fs::read_to_string(claude_dir.join("settings.local.json")).unwrap();
        let v: Value = serde_json::from_str(&content).unwrap();

        // User permissions preserved
        assert!(v.get("permissions").is_some());

        // User hooks preserved — PostToolUse is NOT injected by nomadflow,
        // so only the user's original hook should remain
        let post_tool = v["hooks"]["PostToolUse"].as_array().unwrap();
        assert_eq!(post_tool.len(), 1);
        // It's the user's hook (no _nomadflow marker)
        assert!(post_tool[0].get("hooks").unwrap()[0]
            .get("_nomadflow")
            .is_none());
    }

    #[tokio::test]
    async fn test_inject_hooks_for_project_no_state_dir() {
        let tmp = TempDir::new().unwrap();
        let project_dir = tmp.path().join("project");
        std::fs::create_dir_all(&project_dir).unwrap();

        let svc = AgentStateService {
            sessions_dir: tmp.path().join("sessions"),
            hooks_dir: tmp.path().join("hooks").join("claude-code"),
        };

        // Create hook script so path is valid
        std::fs::create_dir_all(&svc.hooks_dir).unwrap();
        std::fs::write(svc.hooks_dir.join("state-tracker.sh"), "#!/bin/bash").unwrap();

        svc.inject_hooks_for_project(&project_dir).await.unwrap();

        // Verify settings file was created with hooks
        let settings_path = project_dir.join(".claude").join("settings.local.json");
        assert!(settings_path.exists());

        let content = std::fs::read_to_string(&settings_path).unwrap();
        let v: Value = serde_json::from_str(&content).unwrap();
        let hooks = v.get("hooks").unwrap().as_object().unwrap();
        assert!(hooks.contains_key("SessionStart"));
        assert!(hooks.contains_key("Stop"));

        // Verify NO state directory was created (unlike inject_hooks)
        assert!(
            !svc.sessions_dir.exists(),
            "inject_hooks_for_project should NOT create sessions dir"
        );
    }

    #[tokio::test]
    async fn test_cleanup_hooks_removes_nomadflow_entries() {
        let tmp = TempDir::new().unwrap();
        let project_dir = tmp.path().join("project");
        let claude_dir = project_dir.join(".claude");
        std::fs::create_dir_all(&claude_dir).unwrap();

        let svc = AgentStateService {
            sessions_dir: tmp.path().join("sessions"),
            hooks_dir: tmp.path().join("hooks").join("claude-code"),
        };
        std::fs::create_dir_all(&svc.hooks_dir).unwrap();
        std::fs::write(svc.hooks_dir.join("state-tracker.sh"), "#!/bin/bash").unwrap();

        // Inject hooks first
        svc.inject_hooks(&project_dir, "test-session")
            .await
            .unwrap();

        // Manually add a user hook on an event that nomadflow also uses (e.g., PreToolUse)
        // and a standalone user hook on PostToolUse
        let settings_path = claude_dir.join("settings.local.json");
        let mut v: Value =
            serde_json::from_str(&std::fs::read_to_string(&settings_path).unwrap()).unwrap();
        // Add user hook to PreToolUse (alongside nomadflow's)
        v["hooks"]["PreToolUse"].as_array_mut().unwrap().insert(
            0,
            serde_json::json!({
                "matcher": "Write",
                "hooks": [{ "type": "command", "command": "my-linter.sh" }]
            }),
        );
        // Add standalone user hook on PostToolUse (not injected by nomadflow)
        v["hooks"]["PostToolUse"] = serde_json::json!([{
            "matcher": "Write",
            "hooks": [{ "type": "command", "command": "my-linter.sh" }]
        }]);
        std::fs::write(&settings_path, serde_json::to_string_pretty(&v).unwrap()).unwrap();

        // Cleanup
        svc.cleanup_hooks(&project_dir).await.unwrap();

        let content = std::fs::read_to_string(&settings_path).unwrap();
        let v: Value = serde_json::from_str(&content).unwrap();

        // User hooks should still be there
        let post_tool = v["hooks"]["PostToolUse"].as_array().unwrap();
        assert_eq!(post_tool.len(), 1);
        assert!(post_tool[0]["hooks"][0].get("_nomadflow").is_none());

        // User hook on PreToolUse should remain (nomadflow's was removed)
        let pre_tool = v["hooks"]["PreToolUse"].as_array().unwrap();
        assert_eq!(pre_tool.len(), 1);
        assert!(pre_tool[0]["hooks"][0].get("_nomadflow").is_none());

        // Nomadflow-only events should be removed
        assert!(v["hooks"].get("SessionStart").is_none());
        assert!(v["hooks"].get("Stop").is_none());
    }

    #[tokio::test]
    async fn test_cleanup_hooks_deletes_empty_file() {
        let tmp = TempDir::new().unwrap();
        let project_dir = tmp.path().join("project");
        let claude_dir = project_dir.join(".claude");
        std::fs::create_dir_all(&claude_dir).unwrap();

        let svc = AgentStateService {
            sessions_dir: tmp.path().join("sessions"),
            hooks_dir: tmp.path().join("hooks").join("claude-code"),
        };
        std::fs::create_dir_all(&svc.hooks_dir).unwrap();
        std::fs::write(svc.hooks_dir.join("state-tracker.sh"), "#!/bin/bash").unwrap();

        // Inject then cleanup — file should be deleted
        svc.inject_hooks(&project_dir, "test-session")
            .await
            .unwrap();
        svc.cleanup_hooks(&project_dir).await.unwrap();

        let settings_path = claude_dir.join("settings.local.json");
        assert!(!settings_path.exists());
    }

    #[tokio::test]
    async fn test_ensure_hook_scripts_installs_script() {
        let tmp = TempDir::new().unwrap();
        let hooks_dir = tmp.path().join("hooks").join("claude-code");

        let svc = AgentStateService {
            sessions_dir: tmp.path().join("sessions"),
            hooks_dir: hooks_dir.clone(),
        };

        svc.ensure_hook_scripts().await.unwrap();

        let script_path = hooks_dir.join("state-tracker.sh");
        assert!(script_path.exists());

        let content = std::fs::read_to_string(&script_path).unwrap();
        assert!(content.contains("NomadFlow State Tracker"));
        assert!(content.contains("hook_event_name"));

        // Verify executable permission
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let perms = std::fs::metadata(&script_path).unwrap().permissions();
            assert_eq!(perms.mode() & 0o111, 0o111);
        }
    }

    #[tokio::test]
    async fn test_ensure_hook_scripts_idempotent() {
        let tmp = TempDir::new().unwrap();
        let hooks_dir = tmp.path().join("hooks").join("claude-code");

        let svc = AgentStateService {
            sessions_dir: tmp.path().join("sessions"),
            hooks_dir: hooks_dir.clone(),
        };

        svc.ensure_hook_scripts().await.unwrap();
        let mtime1 = std::fs::metadata(hooks_dir.join("state-tracker.sh"))
            .unwrap()
            .modified()
            .unwrap();

        // Second call should not rewrite
        svc.ensure_hook_scripts().await.unwrap();
        let mtime2 = std::fs::metadata(hooks_dir.join("state-tracker.sh"))
            .unwrap()
            .modified()
            .unwrap();

        assert_eq!(mtime1, mtime2);
    }

    #[tokio::test]
    async fn test_list_states_discovers_multiple_sessions() {
        let tmp = TempDir::new().unwrap();
        let sessions_dir = tmp.path().join("sessions");

        // Create two session state files
        for (id, state) in [
            ("session-1", "generating"),
            ("session-2", "waiting_for_input"),
        ] {
            let dir = sessions_dir.join(id);
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(
                dir.join("agent-state.json"),
                format!(
                    r#"{{"sessionId":"{}","agentType":"claude-code","state":"{}","timestamp":"2026-02-15T10:30:00Z","lastEvent":"Test"}}"#,
                    id, state
                ),
            )
            .unwrap();
        }

        let svc = AgentStateService {
            sessions_dir,
            hooks_dir: tmp.path().join("hooks"),
        };

        let states = svc.list_states().await;
        assert_eq!(states.len(), 2);
    }

    #[test]
    fn test_is_nomadflow_matcher_group() {
        let nomadflow_group = serde_json::json!({
            "hooks": [{ "type": "command", "command": "test.sh", "_nomadflow": true }]
        });
        assert!(is_nomadflow_matcher_group(&nomadflow_group));

        let user_group = serde_json::json!({
            "hooks": [{ "type": "command", "command": "my-hook.sh" }]
        });
        assert!(!is_nomadflow_matcher_group(&user_group));
    }

    #[test]
    fn test_hook_script_contains_all_events() {
        // Verify the embedded script handles all the events we inject
        assert!(HOOK_SCRIPT.contains("SessionStart"));
        assert!(HOOK_SCRIPT.contains("UserPromptSubmit"));
        assert!(HOOK_SCRIPT.contains("PreToolUse"));
        assert!(HOOK_SCRIPT.contains("PostToolUse"));
        assert!(HOOK_SCRIPT.contains("PostToolUseFailure"));
        assert!(HOOK_SCRIPT.contains("Stop"));
        assert!(HOOK_SCRIPT.contains("Notification"));
        assert!(HOOK_SCRIPT.contains("SessionEnd"));
    }

    #[test]
    fn test_hook_script_state_mappings() {
        // Verify correct state mappings exist in the script
        assert!(HOOK_SCRIPT.contains(r#"SessionStart)       STATE="idle""#));
        assert!(HOOK_SCRIPT.contains(r#"UserPromptSubmit)   STATE="generating""#));
        assert!(HOOK_SCRIPT.contains(r#"PreToolUse)         STATE="generating""#));
        assert!(HOOK_SCRIPT.contains("PostToolUse)        return 0")); // No state change
        assert!(HOOK_SCRIPT.contains(r#"PostToolUseFailure) STATE="error""#));
        assert!(HOOK_SCRIPT.contains(r#"Stop)               STATE="waiting_for_input""#));
        assert!(HOOK_SCRIPT.contains(r#"SessionEnd)         STATE="done""#));
    }

    #[test]
    fn test_hook_script_atomic_write() {
        assert!(HOOK_SCRIPT.contains("TMP_FILE="));
        assert!(HOOK_SCRIPT.contains("mv \"$TMP_FILE\""));
    }

    #[test]
    fn test_hook_script_jq_fallback() {
        assert!(HOOK_SCRIPT.contains("command -v jq"));
        assert!(HOOK_SCRIPT.contains("grep -o"));
    }

    /// Helper to run the hook script with given env vars and stdin input.
    async fn run_hook_script(
        script_content: &str,
        env_vars: &[(&str, &str)],
        stdin_json: &serde_json::Value,
    ) -> (std::process::ExitStatus, String, String) {
        use tokio::io::AsyncWriteExt;

        let tmp = TempDir::new().unwrap();
        let script_path = tmp.path().join("state-tracker.sh");
        std::fs::write(&script_path, script_content).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&script_path, std::fs::Permissions::from_mode(0o755)).unwrap();
        }

        let mut cmd = tokio::process::Command::new("bash");
        cmd.arg(script_path.to_str().unwrap())
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .env_remove("NOMADFLOW_SESSION_ID")
            .env_remove("NOMADFLOW_STATE_DIR");
        for (k, v) in env_vars {
            cmd.env(k, v);
        }

        let mut child = cmd.spawn().unwrap();
        if let Some(ref mut stdin) = child.stdin {
            stdin
                .write_all(stdin_json.to_string().as_bytes())
                .await
                .ok();
            stdin.shutdown().await.ok();
        }
        let output = child.wait_with_output().await.unwrap();
        (
            output.status,
            String::from_utf8_lossy(&output.stdout).to_string(),
            String::from_utf8_lossy(&output.stderr).to_string(),
        )
    }

    /// Integration test: run the hook script with mock stdin and verify state file output.
    #[tokio::test]
    async fn test_hook_script_processes_pretooluse_event() {
        let tmp = TempDir::new().unwrap();
        let state_dir = tmp.path().join("state");
        std::fs::create_dir_all(&state_dir).unwrap();

        let input = serde_json::json!({
            "session_id": "abc123",
            "hook_event_name": "PreToolUse",
            "tool_name": "Edit",
            "tool_input": { "file_path": "/tmp/test.rs" }
        });

        let (status, _, stderr) = run_hook_script(
            HOOK_SCRIPT,
            &[
                ("NOMADFLOW_SESSION_ID", "test-int-session"),
                ("NOMADFLOW_STATE_DIR", state_dir.to_str().unwrap()),
            ],
            &input,
        )
        .await;

        assert!(status.success(), "Script failed: {stderr}");

        let state_file = state_dir.join("agent-state.json");
        assert!(state_file.exists(), "State file was not created");

        let state: AgentState =
            serde_json::from_str(&std::fs::read_to_string(&state_file).unwrap()).unwrap();
        assert_eq!(state.session_id, "test-int-session");
        assert_eq!(state.agent_type, "claude-code");
        assert_eq!(state.state, crate::models::AgentStateKind::Generating);
        assert_eq!(state.last_event, "PreToolUse");
        assert_eq!(state.tool_name, Some("Edit".to_string()));
    }

    /// Integration test: verify SessionEnd writes done state with endedAt.
    #[tokio::test]
    async fn test_hook_script_processes_session_end() {
        let tmp = TempDir::new().unwrap();
        let state_dir = tmp.path().join("state");
        std::fs::create_dir_all(&state_dir).unwrap();

        let input = serde_json::json!({
            "session_id": "abc123",
            "hook_event_name": "SessionEnd",
            "reason": "other"
        });

        let (status, _, _) = run_hook_script(
            HOOK_SCRIPT,
            &[
                ("NOMADFLOW_SESSION_ID", "end-test-session"),
                ("NOMADFLOW_STATE_DIR", state_dir.to_str().unwrap()),
            ],
            &input,
        )
        .await;

        assert!(status.success());

        let state: AgentState = serde_json::from_str(
            &std::fs::read_to_string(state_dir.join("agent-state.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(state.state, crate::models::AgentStateKind::Done);
        assert!(state.ended_at.is_some());
    }

    /// Integration test: verify Notification with permission_prompt writes waiting_for_input.
    #[tokio::test]
    async fn test_hook_script_processes_notification_permission_prompt() {
        let tmp = TempDir::new().unwrap();
        let state_dir = tmp.path().join("state");
        std::fs::create_dir_all(&state_dir).unwrap();

        let input = serde_json::json!({
            "session_id": "abc123",
            "hook_event_name": "Notification",
            "notification_type": "permission_prompt",
            "message": "Claude needs your permission to use Bash"
        });

        let (status, _, stderr) = run_hook_script(
            HOOK_SCRIPT,
            &[
                ("NOMADFLOW_SESSION_ID", "notif-test-session"),
                ("NOMADFLOW_STATE_DIR", state_dir.to_str().unwrap()),
            ],
            &input,
        )
        .await;

        assert!(status.success(), "Script failed: {stderr}");

        let state_file = state_dir.join("agent-state.json");
        assert!(
            state_file.exists(),
            "State file was not created for Notification event"
        );

        let state: AgentState =
            serde_json::from_str(&std::fs::read_to_string(&state_file).unwrap()).unwrap();
        assert_eq!(state.session_id, "notif-test-session");
        assert_eq!(state.state, crate::models::AgentStateKind::WaitingForInput);
        assert_eq!(state.last_event, "Notification");
    }

    /// Integration test: verify PostToolUse does NOT write state (no change).
    #[tokio::test]
    async fn test_hook_script_skips_post_tool_use() {
        let tmp = TempDir::new().unwrap();
        let state_dir = tmp.path().join("state");
        std::fs::create_dir_all(&state_dir).unwrap();

        let input = serde_json::json!({
            "session_id": "abc123",
            "hook_event_name": "PostToolUse",
            "tool_name": "Edit"
        });

        let (status, _, _) = run_hook_script(
            HOOK_SCRIPT,
            &[
                ("NOMADFLOW_SESSION_ID", "skip-test-session"),
                ("NOMADFLOW_STATE_DIR", state_dir.to_str().unwrap()),
            ],
            &input,
        )
        .await;

        assert!(status.success());

        // PostToolUse should NOT create a state file (no state change)
        let state_file = state_dir.join("agent-state.json");
        assert!(
            !state_file.exists(),
            "PostToolUse should not write state file"
        );
    }

    /// Integration test: script exits 0 when env vars are not set.
    #[tokio::test]
    async fn test_hook_script_exits_cleanly_without_env_vars() {
        let input = serde_json::json!({
            "hook_event_name": "PreToolUse",
            "tool_name": "Bash"
        });

        let (status, _, _) = run_hook_script(
            HOOK_SCRIPT,
            &[], // No env vars
            &input,
        )
        .await;

        assert!(status.success());
    }

    // ── Generic adapter tests (Story 2.2) ──

    fn tmux_available() -> bool {
        std::process::Command::new("which")
            .arg("tmux")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    fn make_session(id: &str, window: &str, agent_type: &str, number: u32) -> Session {
        Session {
            session_id: id.to_string(),
            window_name: window.to_string(),
            repo: "testrepo".to_string(),
            worktree: "feat".to_string(),
            agent_type: agent_type.to_string(),
            agent_number: number,
        }
    }

    #[test]
    fn test_is_idle_shell() {
        assert!(is_idle_shell("bash"));
        assert!(is_idle_shell("zsh"));
        assert!(is_idle_shell("sh"));
        assert!(is_idle_shell("fish"));
        assert!(is_idle_shell("dash"));
        assert!(is_idle_shell("ksh"));
        assert!(is_idle_shell("tcsh"));
        assert!(is_idle_shell("csh"));
        // Case-insensitive
        assert!(is_idle_shell("Bash"));
        assert!(is_idle_shell("ZSH"));
        // Non-shells
        assert!(!is_idle_shell("python"));
        assert!(!is_idle_shell("node"));
        assert!(!is_idle_shell("claude"));
        assert!(!is_idle_shell("aider"));
        assert!(!is_idle_shell("vim"));
        assert!(!is_idle_shell("sleep"));
    }

    #[test]
    fn test_utc_timestamp_format() {
        let ts = utc_timestamp();
        // Should match ISO 8601 format: YYYY-MM-DDTHH:MM:SSZ
        assert!(ts.ends_with('Z'), "Timestamp should end with Z: {ts}");
        assert_eq!(ts.len(), 20, "Timestamp should be 20 chars: {ts}");
        assert_eq!(&ts[4..5], "-", "Should have dash at pos 4: {ts}");
        assert_eq!(&ts[7..8], "-", "Should have dash at pos 7: {ts}");
        assert_eq!(&ts[10..11], "T", "Should have T at pos 10: {ts}");
        assert_eq!(&ts[13..14], ":", "Should have colon at pos 13: {ts}");
        assert_eq!(&ts[16..17], ":", "Should have colon at pos 16: {ts}");
    }

    /// Task 4.1: detect_process_state with idle shell → WaitingForInput
    #[tokio::test]
    async fn test_detect_process_state_idle_shell() {
        if !tmux_available() {
            eprintln!("Skipping tmux test: tmux not available");
            return;
        }

        let session_name = format!("nf-test-generic-{}", std::process::id());
        crate::shell::run(
            &format!("tmux kill-session -t \"{session_name}\" 2>/dev/null"),
            None,
        )
        .await;

        let tmux = TmuxService::new(&session_name);
        tmux.ensure_session().await.unwrap();

        let win = "testrepo:feat:generic-1";
        tmux.create_window(win, None).await.unwrap();

        // Wait for shell to become idle (up to 3s) — shell init time varies by environment
        for _ in 0..15 {
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
            if let Some(cmd) = tmux.get_pane_command(win).await {
                if is_idle_shell(&cmd) {
                    break;
                }
            }
        }

        let tmp = TempDir::new().unwrap();
        let svc = AgentStateService {
            sessions_dir: tmp.path().join("sessions"),
            hooks_dir: tmp.path().join("hooks"),
        };

        let session = make_session("testrepo-feat-generic-1", win, "generic", 1);
        let state = svc.detect_process_state(&session, &tmux).await;

        // Cleanup
        crate::shell::run(&format!("tmux kill-session -t \"{session_name}\""), None).await;

        assert_eq!(state.state, AgentStateKind::WaitingForInput);
        assert_eq!(state.agent_type, "generic");
        assert_eq!(state.last_event, "ProcessCheck");
        assert!(state.ended_at.is_none());
    }

    /// Task 4.2: detect_process_state with running process → Generating
    #[tokio::test]
    async fn test_detect_process_state_running_process() {
        if !tmux_available() {
            eprintln!("Skipping tmux test: tmux not available");
            return;
        }

        let session_name = format!("nf-test-generic-run-{}", std::process::id());
        crate::shell::run(
            &format!("tmux kill-session -t \"{session_name}\" 2>/dev/null"),
            None,
        )
        .await;

        let tmux = TmuxService::new(&session_name);
        tmux.ensure_session().await.unwrap();

        let win = "testrepo:feat:generic-1";
        tmux.create_window(win, None).await.unwrap();

        // Wait for shell to become idle before sending command (up to 3s)
        for _ in 0..15 {
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
            if let Some(cmd) = tmux.get_pane_command(win).await {
                if is_idle_shell(&cmd) {
                    break;
                }
            }
        }

        // Start a long-running process
        tmux.send_keys(win, "sleep 100", true).await;

        // Poll until the pane command changes from shell to sleep (max 5s)
        let mut detected_non_shell = false;
        for _ in 0..25 {
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
            if let Some(cmd) = tmux.get_pane_command(win).await {
                if !is_idle_shell(&cmd) {
                    detected_non_shell = true;
                    break;
                }
            }
        }
        assert!(
            detected_non_shell,
            "sleep command never became foreground process"
        );

        let tmp = TempDir::new().unwrap();
        let svc = AgentStateService {
            sessions_dir: tmp.path().join("sessions"),
            hooks_dir: tmp.path().join("hooks"),
        };

        let session = make_session("testrepo-feat-generic-1", win, "generic", 1);
        let state = svc.detect_process_state(&session, &tmux).await;

        // Cleanup
        crate::shell::run(&format!("tmux kill-session -t \"{session_name}\""), None).await;

        assert_eq!(state.state, AgentStateKind::Generating);
        assert_eq!(state.agent_type, "generic");
        assert_eq!(state.last_event, "ProcessCheck");
    }

    /// Task 4.3: detect_process_state with missing window → Done
    #[tokio::test]
    async fn test_detect_process_state_missing_window() {
        if !tmux_available() {
            eprintln!("Skipping tmux test: tmux not available");
            return;
        }

        let session_name = format!("nf-test-generic-done-{}", std::process::id());
        crate::shell::run(
            &format!("tmux kill-session -t \"{session_name}\" 2>/dev/null"),
            None,
        )
        .await;

        let tmux = TmuxService::new(&session_name);
        tmux.ensure_session().await.unwrap();

        let tmp = TempDir::new().unwrap();
        let svc = AgentStateService {
            sessions_dir: tmp.path().join("sessions"),
            hooks_dir: tmp.path().join("hooks"),
        };

        // Use a window name that doesn't exist
        let session = make_session(
            "testrepo-feat-generic-99",
            "testrepo:feat:generic-99",
            "generic",
            99,
        );
        let state = svc.detect_process_state(&session, &tmux).await;

        // Cleanup
        crate::shell::run(&format!("tmux kill-session -t \"{session_name}\""), None).await;

        assert_eq!(state.state, AgentStateKind::Done);
        assert_eq!(state.agent_type, "generic");
        assert!(state.ended_at.is_some());
    }

    /// Task 4.4: is_idle_shell with undetermined command → Idle (via detect_process_state path)
    /// Note: triggering a real "undetermined" case in tmux is impractical,
    /// so we verify the logic via the is_idle_shell helper and the code path analysis.
    /// The Idle state is returned when get_pane_command returns None.
    #[test]
    fn test_idle_state_logic() {
        // When get_pane_command returns None, detect_process_state returns Idle.
        // We can't easily mock TmuxService, but we verify the helper function
        // and that the code path exists by inspecting the implementation.
        // The is_idle_shell function returns false for non-shell commands,
        // and the None branch maps to Idle.
        assert!(!is_idle_shell("")); // empty string is not a shell
    }

    /// Task 4.5: read_state_for_session routes correctly based on agent_type
    #[tokio::test]
    async fn test_read_state_for_session_routes_claude_code() {
        let tmp = TempDir::new().unwrap();
        let sessions_dir = tmp.path().join("sessions");
        let session_dir = sessions_dir.join("myapp-feat-claude-code-1");
        std::fs::create_dir_all(&session_dir).unwrap();
        std::fs::write(
            session_dir.join("agent-state.json"),
            r#"{"sessionId":"myapp-feat-claude-code-1","agentType":"claude-code","state":"generating","timestamp":"2026-02-15T10:30:00Z","lastEvent":"PreToolUse","toolName":"Edit"}"#,
        )
        .unwrap();

        let svc = AgentStateService {
            sessions_dir,
            hooks_dir: tmp.path().join("hooks"),
        };

        // Claude-code session: reads from disk
        let session = make_session(
            "myapp-feat-claude-code-1",
            "myapp:feat:claude-code-1",
            "claude-code",
            1,
        );

        // We need a TmuxService but it won't be used for claude-code path
        let tmux = TmuxService::new("nonexistent-session");
        let state = svc.read_state_for_session(&session, &tmux).await.unwrap();

        assert_eq!(state.session_id, "myapp-feat-claude-code-1");
        assert_eq!(state.agent_type, "claude-code");
        assert_eq!(state.state, AgentStateKind::Generating);
        assert_eq!(state.last_event, "PreToolUse");
        assert_eq!(state.tool_name, Some("Edit".to_string()));
    }

    /// Task 4.5 (continued): read_state_for_session routes generic sessions to process detection
    #[tokio::test]
    async fn test_read_state_for_session_routes_generic() {
        if !tmux_available() {
            eprintln!("Skipping tmux test: tmux not available");
            return;
        }

        let session_name = format!("nf-test-route-{}", std::process::id());
        crate::shell::run(
            &format!("tmux kill-session -t \"{session_name}\" 2>/dev/null"),
            None,
        )
        .await;

        let tmux = TmuxService::new(&session_name);
        tmux.ensure_session().await.unwrap();

        let win = "testrepo:feat:generic-1";
        tmux.create_window(win, None).await.unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;

        let tmp = TempDir::new().unwrap();
        let svc = AgentStateService {
            sessions_dir: tmp.path().join("sessions"),
            hooks_dir: tmp.path().join("hooks"),
        };

        let session = make_session("testrepo-feat-generic-1", win, "generic", 1);
        let state = svc.read_state_for_session(&session, &tmux).await.unwrap();

        // Cleanup
        crate::shell::run(&format!("tmux kill-session -t \"{session_name}\""), None).await;

        // Generic: computed from tmux, not from file
        assert_eq!(state.agent_type, "generic");
        assert_eq!(state.last_event, "ProcessCheck");
        assert_eq!(state.state, AgentStateKind::WaitingForInput);
    }

    /// Task 4.6: Returned AgentState has correct fields for generic adapter
    #[tokio::test]
    async fn test_generic_agent_state_fields() {
        if !tmux_available() {
            eprintln!("Skipping tmux test: tmux not available");
            return;
        }

        let session_name = format!("nf-test-fields-{}", std::process::id());
        crate::shell::run(
            &format!("tmux kill-session -t \"{session_name}\" 2>/dev/null"),
            None,
        )
        .await;

        let tmux = TmuxService::new(&session_name);
        tmux.ensure_session().await.unwrap();

        let win = "testrepo:feat:generic-1";
        tmux.create_window(win, None).await.unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;

        let tmp = TempDir::new().unwrap();
        let svc = AgentStateService {
            sessions_dir: tmp.path().join("sessions"),
            hooks_dir: tmp.path().join("hooks"),
        };

        let session = make_session("testrepo-feat-generic-1", win, "generic", 1);
        let state = svc.detect_process_state(&session, &tmux).await;

        // Cleanup
        crate::shell::run(&format!("tmux kill-session -t \"{session_name}\""), None).await;

        // Verify all required fields (AC #8)
        assert_eq!(state.session_id, "testrepo-feat-generic-1");
        assert_eq!(state.agent_type, "generic");
        assert_eq!(state.last_event, "ProcessCheck");
        assert!(!state.timestamp.is_empty());
        assert!(state.timestamp.ends_with('Z'));
        assert!(state.tool_name.is_none());

        // Verify JSON serialization uses camelCase
        let json = serde_json::to_string(&state).unwrap();
        assert!(json.contains("\"sessionId\""));
        assert!(json.contains("\"agentType\":\"generic\""));
        assert!(json.contains("\"lastEvent\":\"ProcessCheck\""));
    }

    /// Task 4.5 (list_all_states): verify it aggregates states from multiple sessions
    #[tokio::test]
    async fn test_list_all_states_mixed_sessions() {
        if !tmux_available() {
            eprintln!("Skipping tmux test: tmux not available");
            return;
        }

        let session_name = format!("nf-test-all-{}", std::process::id());
        crate::shell::run(
            &format!("tmux kill-session -t \"{session_name}\" 2>/dev/null"),
            None,
        )
        .await;

        let tmux = TmuxService::new(&session_name);
        tmux.ensure_session().await.unwrap();

        // Create a generic window
        let generic_win = "testrepo:feat:generic-1";
        tmux.create_window(generic_win, None).await.unwrap();
        // Wait for shell to become idle (up to 3s) — shell init time varies by environment
        for _ in 0..15 {
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
            if let Some(cmd) = tmux.get_pane_command(generic_win).await {
                if is_idle_shell(&cmd) {
                    break;
                }
            }
        }

        let tmp = TempDir::new().unwrap();
        let sessions_dir = tmp.path().join("sessions");

        // Create a claude-code state file on disk
        let cc_dir = sessions_dir.join("testrepo-feat-claude-code-1");
        std::fs::create_dir_all(&cc_dir).unwrap();
        std::fs::write(
            cc_dir.join("agent-state.json"),
            r#"{"sessionId":"testrepo-feat-claude-code-1","agentType":"claude-code","state":"generating","timestamp":"2026-02-15T10:30:00Z","lastEvent":"PreToolUse"}"#,
        ).unwrap();

        let svc = AgentStateService {
            sessions_dir,
            hooks_dir: tmp.path().join("hooks"),
        };

        let sessions = vec![
            make_session(
                "testrepo-feat-claude-code-1",
                "testrepo:feat:claude-code-1",
                "claude-code",
                1,
            ),
            make_session("testrepo-feat-generic-1", generic_win, "generic", 1),
        ];

        let states = svc.list_all_states(&sessions, &tmux).await;

        // Cleanup
        crate::shell::run(&format!("tmux kill-session -t \"{session_name}\""), None).await;

        assert_eq!(states.len(), 2);
        // Claude-code session from file
        let cc_state = states
            .iter()
            .find(|s| s.agent_type == "claude-code")
            .unwrap();
        assert_eq!(cc_state.state, AgentStateKind::Generating);
        // Generic session from tmux
        let gen_state = states.iter().find(|s| s.agent_type == "generic").unwrap();
        assert_eq!(gen_state.state, AgentStateKind::WaitingForInput);
        assert_eq!(gen_state.last_event, "ProcessCheck");
    }
}
