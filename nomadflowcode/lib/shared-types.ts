export interface Server {
  id: string;
  name: string;
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
  worktreeName?: string;
}

export interface PTYPane {
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
  worktreeName: string;
}

export interface CreateFeatureResult {
  worktreePath: string;
  branch: string;
  worktreeName: string;
}

export interface BranchInfo {
  name: string;
  isRemote: boolean;
  remoteName?: string;
}

export interface BranchListResponse {
  branches: BranchInfo[];
  defaultBranch: string;
}

export interface AttachBranchResult {
  worktreePath: string;
  branch: string;
  worktreeName: string;
}

// Git diff/status types (matches Rust models in nomadflow-core/src/models.rs)
export type FileChangeStatus = 'modified' | 'new' | 'deleted' | 'renamed' | 'conflicted';

export interface FileChange {
  path: string;
  status: FileChangeStatus;
  additions: number;
  deletions: number;
}

export interface StatusSummary {
  modified: number;
  new: number;
  deleted: number;
  conflicted: number;
  totalAdditions: number;
  totalDeletions: number;
}

export interface WorktreeStatusResponse {
  worktreePath: string;
  branch: string;
  files: FileChange[];
  summary: StatusSummary;
}

export interface FileDiffResponse {
  filePath: string;
  hunks: DiffHunk[];
}

export interface DiffHunk {
  oldStart: number;
  oldLines: number;
  newStart: number;
  newLines: number;
  lines: DiffLine[];
}

export type DiffLineType = 'context' | 'addition' | 'deletion';

export interface DiffLine {
  type: DiffLineType;
  content: string;
}

export interface FileContentResponse {
  filePath: string;
  content: string;
}

export interface DirEntry {
  name: string;
  path: string;
  isDir: boolean;
  size: number;
}

export interface ListDirResponse {
  entries: DirEntry[];
  path: string;
}

// Re-export session + agent state types
export type { AgentStateKind, SessionWithState, ListSessionsResponse } from './types/session';
