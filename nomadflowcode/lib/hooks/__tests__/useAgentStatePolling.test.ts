import { renderHook, act, waitFor } from '@testing-library/react-native';
import { useAgentStatePolling } from '@/lib/hooks/useAgentStatePolling';
import type { ListSessionsResponse } from '@/lib/types/session';

const mockFetch = global.fetch as jest.Mock;

const SERVER_URL = 'http://localhost:8080';
const SECRET = 'test-secret';

function mockSuccessResponse(sessions: ListSessionsResponse['sessions'] = []) {
  mockFetch.mockResolvedValueOnce({
    ok: true,
    json: async () => ({ sessions }),
  });
}

describe('useAgentStatePolling', () => {
  beforeEach(() => {
    mockFetch.mockReset();
  });

  it('fires initial poll immediately on mount (not after interval)', async () => {
    mockSuccessResponse();

    renderHook(() => useAgentStatePolling(SERVER_URL, SECRET));

    await waitFor(() => {
      expect(mockFetch).toHaveBeenCalledTimes(1);
    });

    expect(mockFetch).toHaveBeenCalledWith(
      'http://localhost:8080/api/list-sessions',
      expect.objectContaining({
        method: 'POST',
        headers: expect.objectContaining({
          Authorization: 'Bearer test-secret',
        }),
      })
    );
  });

  it('fires subsequent polls at configured interval', async () => {
    jest.useFakeTimers();
    mockSuccessResponse();
    mockSuccessResponse();
    mockSuccessResponse();

    renderHook(() => useAgentStatePolling(SERVER_URL, SECRET, { interval: 3000 }));

    // Flush initial poll
    await act(async () => {
      await jest.advanceTimersByTimeAsync(0);
    });
    expect(mockFetch).toHaveBeenCalledTimes(1);

    // Advance by one interval
    await act(async () => {
      await jest.advanceTimersByTimeAsync(3000);
    });
    expect(mockFetch).toHaveBeenCalledTimes(2);

    // Advance by another interval
    await act(async () => {
      await jest.advanceTimersByTimeAsync(3000);
    });
    expect(mockFetch).toHaveBeenCalledTimes(3);

    jest.useRealTimers();
  });

  it('cleans up timeout on unmount', async () => {
    jest.useFakeTimers();
    const clearTimeoutSpy = jest.spyOn(global, 'clearTimeout');
    mockSuccessResponse();

    const { unmount } = renderHook(() => useAgentStatePolling(SERVER_URL, SECRET));

    // Flush initial poll
    await act(async () => {
      await jest.advanceTimersByTimeAsync(0);
    });

    unmount();

    expect(clearTimeoutSpy).toHaveBeenCalled();

    jest.useRealTimers();
  });

  it('calls onStateChange when session state transitions', async () => {
    jest.useFakeTimers();
    const onStateChange = jest.fn();

    // First poll: session is 'idle'
    mockSuccessResponse([
      {
        sessionId: 'sess-1',
        windowName: 'test',
        repo: 'repo',
        worktree: '/tmp',
        agentType: 'claude',
        agentNumber: 1,
        agentState: 'idle',
        stateTimestamp: null,
      },
    ]);

    renderHook(() => useAgentStatePolling(SERVER_URL, SECRET, { onStateChange }));

    // Flush initial poll
    await act(async () => {
      await jest.advanceTimersByTimeAsync(0);
    });

    // First poll doesn't trigger onStateChange (no previous state)
    expect(onStateChange).not.toHaveBeenCalled();

    // Second poll: session transitions to 'generating'
    mockSuccessResponse([
      {
        sessionId: 'sess-1',
        windowName: 'test',
        repo: 'repo',
        worktree: '/tmp',
        agentType: 'claude',
        agentNumber: 1,
        agentState: 'generating',
        stateTimestamp: null,
      },
    ]);

    await act(async () => {
      await jest.advanceTimersByTimeAsync(3000);
    });

    expect(onStateChange).toHaveBeenCalledWith('sess-1', 'idle', 'generating');

    jest.useRealTimers();
  });

  it('sets error state on fetch failure', async () => {
    mockFetch.mockRejectedValueOnce(new Error('Network error'));

    const { result } = renderHook(() => useAgentStatePolling(SERVER_URL, SECRET));

    await waitFor(() => {
      expect(result.current.error).toBe('Network error');
      expect(result.current.loading).toBe(false);
    });
  });
});
