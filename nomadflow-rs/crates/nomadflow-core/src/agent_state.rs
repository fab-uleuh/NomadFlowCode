use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::config::Settings;
use crate::error::{NomadError, Result};
use crate::models::AgentState;

/// Embedded hook script content — installed to ~/.nomadflowcode/hooks/claude-code/state-tracker.sh
const HOOK_SCRIPT: &str = r#"#!/bin/bash
# NomadFlow State Tracker — CWD-based agent state detection.
# IMPORTANT: Always exit 0 to never block Claude's execution.

INPUT=$(cat)
if command -v jq >/dev/null 2>&1; then
    PARSED=$(echo "$INPUT" | jq -r '[(.hook_event_name // ""), (.cwd // ""), (.notification_type // "")] | @tsv')
    EVENT=$(echo "$PARSED" | cut -f1); CWD=$(echo "$PARSED" | cut -f2); NTYPE=$(echo "$PARSED" | cut -f3)
else
    EVENT=$(echo "$INPUT" | grep -o '"hook_event_name"[[:space:]]*:[[:space:]]*"[^"]*"' | head -1 | grep -o '"[^"]*"$' | tr -d '"')
    CWD=$(echo "$INPUT" | grep -o '"cwd"[[:space:]]*:[[:space:]]*"[^"]*"' | head -1 | grep -o '"[^"]*"$' | tr -d '"')
    NTYPE=$(echo "$INPUT" | grep -o '"notification_type"[[:space:]]*:[[:space:]]*"[^"]*"' | head -1 | grep -o '"[^"]*"$' | tr -d '"')
fi

[ -z "$EVENT" ] || [ -z "$CWD" ] && exit 0

STATE=""
case "$EVENT" in
    UserPromptSubmit|PreToolUse) STATE="generating" ;;
    Stop) STATE="waiting_for_input" ;;
    Notification) case "$NTYPE" in permission_prompt) STATE="waiting_for_permission" ;; idle_prompt) STATE="waiting_for_input" ;; *) exit 0 ;; esac ;;
    *) exit 0 ;;
esac

# Canonicalize CWD (handles /tmp -> /private/tmp on macOS)
CWD=$(cd "$CWD" 2>/dev/null && pwd -P || echo "$CWD")
# Collision-free encoding: escape existing dashes, then replace / with -
ENCODED=$(echo "$CWD" | sed 's/-/--/g' | tr '/' '-' | sed 's/^-//')
# Derive STATE_DIR from script's own location ({base_dir}/hooks/claude-code/state-tracker.sh)
STATE_DIR="$(cd "$(dirname "$0")/../.." 2>/dev/null && pwd)/agent-states"
mkdir -p "$STATE_DIR"
TIMESTAMP=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
TMP="${STATE_DIR}/${ENCODED}.json.tmp.$$"
echo "{\"state\":\"$STATE\",\"timestamp\":\"$TIMESTAMP\",\"lastEvent\":\"$EVENT\"}" > "$TMP"
mv "$TMP" "${STATE_DIR}/${ENCODED}.json"
exit 0
"#;

/// Hook events that nomadflow injects into .claude/settings.local.json.
const HOOK_EVENTS: &[(&str, Option<&str>)] = &[
    ("UserPromptSubmit", None),
    ("PreToolUse", None),
    ("Stop", None),
    ("Notification", None),
];

/// Service for managing agent state detection via Claude Code hooks.
pub struct AgentStateService {
    agent_states_dir: PathBuf,
    hooks_dir: PathBuf,
}

impl AgentStateService {
    pub fn new(settings: &Settings) -> Self {
        Self {
            agent_states_dir: settings.base_dir().join("agent-states"),
            hooks_dir: settings.base_dir().join("hooks").join("claude-code"),
        }
    }

    /// Install/update hook scripts to ~/.nomadflowcode/hooks/claude-code/.
    /// Idempotent — overwrites existing script if content differs.
    /// Also ensures agent-states directory exists.
    pub async fn ensure_hook_scripts(&self) -> Result<()> {
        tokio::fs::create_dir_all(&self.hooks_dir).await?;
        tokio::fs::create_dir_all(&self.agent_states_dir).await?;

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
    pub async fn inject_hooks(&self, project_dir: &Path) -> Result<()> {
        self.inject_hooks_impl(project_dir).await
    }

    /// Read agent state for a given CWD path.
    /// Canonicalizes the CWD (handles macOS /tmp -> /private/tmp), then
    /// encodes it as a collision-free filename.
    pub async fn get_state_by_cwd(&self, cwd: &str) -> Result<Option<AgentState>> {
        // Canonicalize to match the bash hook's `cd && pwd -P`
        let canonical = std::path::Path::new(cwd)
            .canonicalize()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| cwd.to_string());
        let encoded = Self::encode_cwd(&canonical);
        let state_path = self.agent_states_dir.join(format!("{encoded}.json"));
        match tokio::fs::read_to_string(&state_path).await {
            Ok(content) => {
                let state = serde_json::from_str(&content).map_err(|e| {
                    NomadError::Config(format!("Failed to parse agent state file: {e}"))
                })?;
                Ok(Some(state))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Encode a CWD path for use as a state file name.
    /// Collision-free: escapes existing `-` as `--`, then replaces `/` with `-`.
    pub fn encode_cwd(cwd: &str) -> String {
        let escaped = cwd.replace('-', "--");
        let encoded = escaped.replace('/', "-");
        encoded.strip_prefix('-').unwrap_or(&encoded).to_string()
    }

    /// Internal: inject hooks into .claude/settings.local.json.
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

    /// Delete the state file for a given CWD.
    pub async fn delete_state_file(&self, cwd: &str) {
        let canonical = std::path::Path::new(cwd)
            .canonicalize()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| cwd.to_string());
        let encoded = Self::encode_cwd(&canonical);
        let path = self.agent_states_dir.join(format!("{encoded}.json"));
        tokio::fs::remove_file(&path).await.ok();
    }

    /// Remove state files older than `max_age` from the agent-states directory.
    pub async fn purge_stale_state_files(&self, max_age: std::time::Duration) {
        let mut entries = match tokio::fs::read_dir(&self.agent_states_dir).await {
            Ok(e) => e,
            Err(_) => return,
        };

        let now = std::time::SystemTime::now();

        while let Ok(Some(entry)) = entries.next_entry().await {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            if let Ok(meta) = tokio::fs::metadata(&path).await {
                if let Ok(modified) = meta.modified() {
                    if let Ok(age) = now.duration_since(modified) {
                        if age > max_age {
                            tokio::fs::remove_file(&path).await.ok();
                        }
                    }
                }
            }
        }
    }

    /// Get the agent-states directory path.
    pub fn agent_states_dir(&self) -> &Path {
        &self.agent_states_dir
    }

    /// Get the hooks directory path (for testing).
    pub fn hooks_dir(&self) -> &Path {
        &self.hooks_dir
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

    fn make_svc(tmp: &TempDir) -> AgentStateService {
        AgentStateService {
            agent_states_dir: tmp.path().join("agent-states"),
            hooks_dir: tmp.path().join("hooks").join("claude-code"),
        }
    }

    #[tokio::test]
    async fn test_get_state_by_cwd_valid_file() {
        let tmp = TempDir::new().unwrap();
        let states_dir = tmp.path().join("agent-states");
        std::fs::create_dir_all(&states_dir).unwrap();

        // Use the tmp path itself as CWD (it exists, so canonicalize works)
        let cwd = tmp.path().to_string_lossy().to_string();
        let canonical = std::path::Path::new(&cwd)
            .canonicalize()
            .unwrap()
            .to_string_lossy()
            .to_string();
        let encoded = AgentStateService::encode_cwd(&canonical);
        std::fs::write(
            states_dir.join(format!("{encoded}.json")),
            r#"{"state":"generating","timestamp":"2026-02-15T10:30:00Z","lastEvent":"PreToolUse"}"#,
        )
        .unwrap();

        let svc = make_svc(&tmp);
        let state = svc.get_state_by_cwd(&cwd).await.unwrap().unwrap();
        assert_eq!(state.state, crate::models::AgentStateKind::Generating);
        assert_eq!(state.last_event, "PreToolUse");
    }

    #[tokio::test]
    async fn test_get_state_by_cwd_missing_file() {
        let tmp = TempDir::new().unwrap();
        let svc = make_svc(&tmp);
        assert!(svc.get_state_by_cwd("/nonexistent/path").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_inject_hooks_creates_settings_file() {
        let tmp = TempDir::new().unwrap();
        let project_dir = tmp.path().join("project");
        std::fs::create_dir_all(&project_dir).unwrap();

        let svc = make_svc(&tmp);

        // Create hook script so path is valid
        std::fs::create_dir_all(&svc.hooks_dir).unwrap();
        std::fs::write(svc.hooks_dir.join("state-tracker.sh"), "#!/bin/bash").unwrap();

        svc.inject_hooks(&project_dir).await.unwrap();

        let settings_path = project_dir.join(".claude").join("settings.local.json");
        assert!(settings_path.exists());

        let content = std::fs::read_to_string(&settings_path).unwrap();
        let v: Value = serde_json::from_str(&content).unwrap();

        // Verify the 4 hook events are present
        let hooks = v.get("hooks").unwrap().as_object().unwrap();
        assert!(hooks.contains_key("UserPromptSubmit"));
        assert!(hooks.contains_key("PreToolUse"));
        assert!(hooks.contains_key("Stop"));
        assert!(hooks.contains_key("Notification"));
        // Removed events should NOT be present
        assert!(!hooks.contains_key("SessionStart"));
        assert!(!hooks.contains_key("SessionEnd"));
        assert!(!hooks.contains_key("PostToolUseFailure"));
    }

    #[tokio::test]
    async fn test_cleanup_hooks_removes_nomadflow_entries() {
        let tmp = TempDir::new().unwrap();
        let project_dir = tmp.path().join("project");
        let claude_dir = project_dir.join(".claude");
        std::fs::create_dir_all(&claude_dir).unwrap();

        let svc = make_svc(&tmp);
        std::fs::create_dir_all(&svc.hooks_dir).unwrap();
        std::fs::write(svc.hooks_dir.join("state-tracker.sh"), "#!/bin/bash").unwrap();

        // Inject hooks first
        svc.inject_hooks(&project_dir).await.unwrap();

        // Cleanup
        svc.cleanup_hooks(&project_dir).await.unwrap();

        let settings_path = claude_dir.join("settings.local.json");
        assert!(!settings_path.exists());
    }

    #[tokio::test]
    async fn test_ensure_hook_scripts_installs_script() {
        let tmp = TempDir::new().unwrap();
        let svc = make_svc(&tmp);

        svc.ensure_hook_scripts().await.unwrap();

        let script_path = svc.hooks_dir.join("state-tracker.sh");
        assert!(script_path.exists());

        let content = std::fs::read_to_string(&script_path).unwrap();
        assert!(content.contains("NomadFlow State Tracker"));
    }

    #[test]
    fn test_hook_script_uses_cwd() {
        // Verify script parses cwd from stdin, not session env vars
        assert!(HOOK_SCRIPT.contains("cwd"));
        assert!(!HOOK_SCRIPT.contains("NOMADFLOW_SESSION_ID"));
        assert!(!HOOK_SCRIPT.contains("NOMADFLOW_STATE_DIR"));
    }

    #[test]
    fn test_encode_cwd() {
        // Basic paths
        assert_eq!(AgentStateService::encode_cwd("/Users/dev/myproject"), "Users-dev-myproject");
        assert_eq!(AgentStateService::encode_cwd("/tmp/test"), "tmp-test");
        assert_eq!(AgentStateService::encode_cwd("relative/path"), "relative-path");

        // Collision-free: paths with dashes are escaped
        assert_ne!(
            AgentStateService::encode_cwd("/a/b-c"),
            AgentStateService::encode_cwd("/a/b/c")
        );
        assert_eq!(AgentStateService::encode_cwd("/a/b-c"), "a-b--c");
        assert_eq!(AgentStateService::encode_cwd("/a/b/c"), "a-b-c");

        // Edge cases
        assert_eq!(AgentStateService::encode_cwd(""), "");
        assert_eq!(AgentStateService::encode_cwd("/"), "");
    }

    #[tokio::test]
    async fn test_delete_state_file() {
        let tmp = TempDir::new().unwrap();
        let svc = make_svc(&tmp);

        // Use the tmp path itself as CWD (it exists, so canonicalize works)
        let cwd = tmp.path().to_string_lossy().to_string();
        let canonical = std::path::Path::new(&cwd)
            .canonicalize()
            .unwrap()
            .to_string_lossy()
            .to_string();
        let encoded = AgentStateService::encode_cwd(&canonical);

        let states_dir = tmp.path().join("agent-states");
        std::fs::create_dir_all(&states_dir).unwrap();
        let state_file = states_dir.join(format!("{encoded}.json"));
        std::fs::write(&state_file, r#"{"state":"generating","timestamp":"2026-02-15T10:30:00Z","lastEvent":"PreToolUse"}"#).unwrap();
        assert!(state_file.exists());

        svc.delete_state_file(&cwd).await;
        assert!(!state_file.exists());
    }

    #[tokio::test]
    async fn test_purge_stale_state_files() {
        let tmp = TempDir::new().unwrap();
        let svc = make_svc(&tmp);

        let states_dir = tmp.path().join("agent-states");
        std::fs::create_dir_all(&states_dir).unwrap();

        let content = r#"{"state":"idle","timestamp":"2026-02-15T10:30:00Z","lastEvent":"Stop"}"#;

        // Create a "recent" file
        let recent = states_dir.join("recent.json");
        std::fs::write(&recent, content).unwrap();

        // Create an "old" file and backdate its mtime
        let old = states_dir.join("old.json");
        std::fs::write(&old, content).unwrap();
        let old_time = std::time::SystemTime::now() - std::time::Duration::from_secs(48 * 3600);
        filetime::set_file_mtime(&old, filetime::FileTime::from_system_time(old_time)).unwrap();

        // Non-json file should be ignored
        let other = states_dir.join("notes.txt");
        std::fs::write(&other, "keep me").unwrap();
        filetime::set_file_mtime(&other, filetime::FileTime::from_system_time(old_time)).unwrap();

        svc.purge_stale_state_files(std::time::Duration::from_secs(24 * 3600)).await;

        assert!(recent.exists(), "recent file should survive");
        assert!(!old.exists(), "old file should be purged");
        assert!(other.exists(), "non-json file should be ignored");
    }
}
