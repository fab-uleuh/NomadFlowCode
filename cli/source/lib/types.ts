// Mirror of nomadflowcode/lib/types/index.ts - adapted for CLI

export interface Server {
  id: string;
  name: string;
  ttydUrl?: string;
  apiUrl?: string;
  authToken?: string;
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
  isMain?: boolean;
  createdAt?: number;
  tmuxWindow?: string;
  /** Process running in the tmux pane (detected locally) */
  paneCommand?: string;
}

export interface TmuxWindow {
  index: number;
  name: string;
  active: boolean;
}

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

export interface SwitchFeatureResult {
  switched: boolean;
  worktreePath: string;
  tmuxWindow: string;
  hasRunningProcess: boolean;
}

export interface CreateFeatureResult {
  worktreePath: string;
  branch: string;
  tmuxWindow: string;
}

export interface CliState {
  lastServer?: string;
  lastRepo?: string;
  lastFeature?: string;
  lastAttached?: number;
}

export type WizardStep = 'resume' | 'server' | 'repo' | 'feature' | 'attach';
