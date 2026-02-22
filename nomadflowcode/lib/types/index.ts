import type { Server, Feature } from '@shared';

// Re-export session types
export type { AgentStateKind, SessionWithState, ListSessionsResponse } from './session';

// Navigation types for expo-router
export type RootStackParamList = {
  index: undefined;
  'add-server': { server?: Server };
  repos: { serverId: string };
  features: { serverId: string; repoPath: string };
  terminal: { serverId: string; repoPath: string; featureName: string };
  settings: undefined;
  jobs: { serverId: string };
  'job-detail': { serverId: string; jobId: string };
};

export interface PTYSession {
  name: string;
  panes: import('@shared').PTYPane[];
  attached: boolean;
}

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
  action: 'list-repos' | 'list-features' | 'create-feature' | 'delete-feature' | 'switch-feature' | 'clone-repo' | 'list-branches' | 'attach-branch' | 'create-session' | 'close-session' | 'select-session' | 'worktree-status' | 'file-diff' | 'file-content' | 'list-sessions' | 'list-panes' | 'destroy-pane' | 'list-dir';
  params?: Record<string, unknown>;
}

export interface CreateFeatureParams {
  repoPath: string;
  branchName: string;
  baseBranch?: string;
}

export interface SwitchFeatureParams {
  repoPath: string;
  featureName: string;
  launchAgent?: boolean;
  agentCommand?: string;
  linkedSession?: string;
}

export interface TerminalShortcut {
  id: string;
  label: string;
  command: string;
  autoExecute: boolean;
  order: number;
}
