import type { Server, Feature } from '@shared';

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

export interface TmuxSession {
  name: string;
  windows: import('@shared').TmuxWindow[];
  attached: boolean;
}

export interface AppSettings {
  defaultAiAgent: 'claude' | 'ollama' | 'custom';
  customAgentCommand?: string;
  autoLaunchAgent: boolean;
  tmuxSessionPrefix: string;
  theme: 'dark' | 'light' | 'system';
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

export interface TerminalMessage {
  type: 'input' | 'output' | 'resize' | 'ping' | 'pong';
  data?: string;
  cols?: number;
  rows?: number;
}

// Command types for server scripts
export interface ServerCommand {
  action: 'list-repos' | 'list-features' | 'create-feature' | 'delete-feature' | 'switch-feature' | 'clone-repo' | 'list-branches' | 'attach-branch';
  params?: Record<string, string>;
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
}

export interface TerminalShortcut {
  id: string;
  label: string;
  command: string;
  autoExecute: boolean;
  order: number;
}
