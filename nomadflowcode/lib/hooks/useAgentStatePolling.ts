import { useState, useEffect, useRef, useCallback } from 'react';
import type { SessionWithState, ListSessionsResponse } from '../types/session';

interface UseAgentStatePollingResult {
  sessions: SessionWithState[];
  loading: boolean;
  error: string | null;
}

interface UseAgentStatePollingOptions {
  interval?: number;
  onStateChange?: (
    sessionId: string,
    previous: string,
    current: string
  ) => void;
}

const POLL_TIMEOUT = 10000;

export function useAgentStatePolling(
  serverUrl: string,
  secret: string,
  options: UseAgentStatePollingOptions = {}
): UseAgentStatePollingResult {
  const { interval = 3000 } = options;
  const [sessions, setSessions] = useState<SessionWithState[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const previousStatesRef = useRef<Map<string, string>>(new Map());
  const onStateChangeRef = useRef(options.onStateChange);
  const abortRef = useRef<AbortController | null>(null);

  // Keep onStateChange ref up-to-date without retriggering poll/effect
  useEffect(() => {
    onStateChangeRef.current = options.onStateChange;
  }, [options.onStateChange]);

  const poll = useCallback(async () => {
    // Abort any in-flight request before starting a new one
    abortRef.current?.abort();
    const controller = new AbortController();
    abortRef.current = controller;

    const apiUrl = serverUrl.replace(/\/$/, '');
    const timeoutId = setTimeout(() => controller.abort(), POLL_TIMEOUT);

    try {
      const response = await fetch(`${apiUrl}/api/list-sessions`, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
          Authorization: `Bearer ${secret}`,
        },
        body: JSON.stringify({}),
        signal: controller.signal,
      });

      if (!response.ok) {
        const errorData = await response.json().catch(() => ({}));
        clearTimeout(timeoutId);
        setError(
          errorData.detail || `HTTP ${response.status}: ${response.statusText}`
        );
        setLoading(false);
        return;
      }

      const data: ListSessionsResponse = await response.json();
      clearTimeout(timeoutId);

      // Don't update state if aborted during body read
      if (controller.signal.aborted) return;

      setSessions(data.sessions);
      setError(null);

      // Client-side diffing for state changes
      const onStateChange = onStateChangeRef.current;
      if (onStateChange) {
        const prevStates = previousStatesRef.current;
        const currentIds = new Set<string>();

        for (const session of data.sessions) {
          currentIds.add(session.sessionId);
          const prev = prevStates.get(session.sessionId);
          if (prev && prev !== session.agentState) {
            onStateChange(session.sessionId, prev, session.agentState);
          }
          prevStates.set(session.sessionId, session.agentState);
        }

        // Clean stale entries for sessions that no longer exist
        for (const id of prevStates.keys()) {
          if (!currentIds.has(id)) {
            prevStates.delete(id);
          }
        }
      }
    } catch (err) {
      clearTimeout(timeoutId);
      // Don't update state if intentionally aborted (unmount or new poll)
      if (controller.signal.aborted) return;
      const message = err instanceof Error ? err.message : 'Unknown error';
      setError(
        message === 'The operation was aborted' ? 'Request timeout' : message
      );
    }
    setLoading(false);
  }, [serverUrl, secret]);

  // Reset loading when serverUrl/secret changes (poll identity changes)
  useEffect(() => {
    setLoading(true);
  }, [poll]);

  // setTimeout-loop: re-schedule only after current poll completes (no overlap)
  useEffect(() => {
    let timeoutId: ReturnType<typeof setTimeout> | null = null;
    let cancelled = false;

    const run = async () => {
      await poll();
      if (!cancelled) {
        timeoutId = setTimeout(run, interval);
      }
    };

    run();

    return () => {
      cancelled = true;
      if (timeoutId) clearTimeout(timeoutId);
      abortRef.current?.abort();
    };
  }, [poll, interval]);

  return { sessions, loading, error };
}
