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

// Data types
export interface Server {
  id: string;
  name: string;
  ttydUrl?: string;      // HTTP URL for ttyd terminal - http://host:7681
  apiUrl?: string;       // REST API URL - http://host:8080
  authToken?: string;    // Shared secret for API and ttyd authentication
  lastConnected?: number;
}

export interface Repository {
  name: string;
  path: string;
  branch: string;
  lastAccessed?: number;
}

export interface Feature {
  name: string;
  worktreePath: string;
  branch: string;
  isActive: boolean;
  createdAt?: number;
  tmuxWindow?: string;
}

export interface TmuxSession {
  name: string;
  windows: TmuxWindow[];
  attached: boolean;
}

export interface TmuxWindow {
  index: number;
  name: string;
  active: boolean;
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

// API Response types
export interface ApiResponse<T> {
  success: boolean;
  data?: T;
  error?: string;
}

export interface RepoListResponse {
  repos: Repository[];
}

export interface FeatureListResponse {
  features: Feature[];
}

// Command types for server scripts
export interface ServerCommand {
  action: 'list-repos' | 'list-features' | 'create-feature' | 'delete-feature' | 'switch-feature' | 'clone-repo';
  params?: Record<string, string>;
}

export interface CreateFeatureParams {
  repoPath: string;
  featureName: string;
  baseBranch?: string;
}

export interface SwitchFeatureParams {
  repoPath: string;
  featureName: string;
  launchAgent?: boolean;
  agentCommand?: string;
}

export interface SwitchFeatureResult {
  worktreePath: string;
  tmuxWindow: string;
  hasRunningProcess: boolean;
}
