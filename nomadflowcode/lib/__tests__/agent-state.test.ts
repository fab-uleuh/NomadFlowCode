import { aggregateState } from '@/lib/agent-state';
import type { SessionWithState } from '@/lib/types/session';

function makeSession(agentState: SessionWithState['agentState']): SessionWithState {
  return {
    sessionId: `session-${agentState}`,
    windowName: 'test',
    repo: 'test-repo',
    worktree: '/tmp/test',
    agentType: 'claude',
    agentNumber: 1,
    agentState,
    stateTimestamp: null,
  };
}

describe('aggregateState', () => {
  it('returns "unknown" for empty sessions array', () => {
    expect(aggregateState([])).toBe('unknown');
  });

  it('returns the state of a single session', () => {
    expect(aggregateState([makeSession('generating')])).toBe('generating');
    expect(aggregateState([makeSession('idle')])).toBe('idle');
    expect(aggregateState([makeSession('error')])).toBe('error');
  });

  it('returns highest priority state from mixed sessions', () => {
    const sessions = [makeSession('idle'), makeSession('generating'), makeSession('done')];
    expect(aggregateState(sessions)).toBe('generating');
  });

  it('prioritizes waiting_for_input over all other states', () => {
    const sessions = [
      makeSession('error'),
      makeSession('generating'),
      makeSession('waiting_for_input'),
    ];
    expect(aggregateState(sessions)).toBe('waiting_for_input');
  });

  it('prioritizes error over generating/idle/done/unknown', () => {
    const sessions = [makeSession('generating'), makeSession('error'), makeSession('done')];
    expect(aggregateState(sessions)).toBe('error');
  });

  it('follows full priority order: waiting_for_input > error > generating > idle > done > unknown', () => {
    // Test each adjacent pair to confirm the full ordering via behavior
    expect(aggregateState([makeSession('error'), makeSession('waiting_for_input')])).toBe('waiting_for_input');
    expect(aggregateState([makeSession('generating'), makeSession('error')])).toBe('error');
    expect(aggregateState([makeSession('idle'), makeSession('generating')])).toBe('generating');
    expect(aggregateState([makeSession('done'), makeSession('idle')])).toBe('idle');
    expect(aggregateState([makeSession('unknown'), makeSession('done')])).toBe('done');
  });
});
