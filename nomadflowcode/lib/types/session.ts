export type AgentStateKind =
  | 'waiting_for_input'
  | 'waiting_for_permission'
  | 'generating'
  | 'idle'
  | 'done'
  | 'error'
  | 'unknown';

export interface SessionWithState {
  sessionId: string;
  windowName: string;
  repo: string;
  worktree: string;
  agentType: string;
  agentNumber: number;
  agentState: AgentStateKind;
  stateTimestamp: string | null;
}

export interface ListSessionsResponse {
  sessions: SessionWithState[];
}
