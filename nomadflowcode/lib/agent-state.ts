import type { AgentStateKind, SessionWithState } from '@/lib/types/session';

export const STATE_PRIORITY: AgentStateKind[] = [
  'waiting_for_input',
  'error',
  'generating',
  'idle',
  'done',
  'unknown',
];

export function aggregateState(sessions: SessionWithState[]): AgentStateKind {
  for (const state of STATE_PRIORITY) {
    if (sessions.some((s) => s.agentState === state)) return state;
  }
  return 'unknown';
}
