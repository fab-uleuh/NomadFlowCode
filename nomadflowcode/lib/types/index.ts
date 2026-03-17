// Re-export session types
export type { AgentStateKind, SessionWithState, ListSessionsResponse } from './session';

export interface AppSettings {
  defaultAiAgent: 'claude' | 'ollama' | 'custom';
  customAgentCommand?: string;
  autoLaunchAgent: boolean;
  theme: 'dark' | 'light' | 'system';
  language?: 'en' | 'fr';
  fontSize: number;
  autoReconnect: boolean;
  reconnectDelay: number;
  maxReconnectAttempts: number;
}

export interface ConnectionState {
  status: 'disconnected' | 'connecting' | 'connected' | 'reconnecting' | 'error';
  error?: string;
  reconnectAttempts: number;
}

// Command types for server scripts
export interface ServerCommand {
  action: 'list-repos' | 'list-features' | 'create-feature' | 'delete-feature' | 'switch-feature' | 'clone-repo' | 'list-branches' | 'attach-branch' | 'create-session' | 'close-session' | 'worktree-status' | 'file-diff' | 'file-content' | 'list-sessions' | 'list-panes' | 'destroy-pane' | 'list-dir';
  params?: Record<string, unknown>;
}

export interface TerminalShortcut {
  id: string;
  label: string;
  command: string;
  autoExecute: boolean;
  order: number;
}
