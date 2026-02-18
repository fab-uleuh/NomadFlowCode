import {
  useState,
  useRef,
  useCallback,
  useEffect,
  forwardRef,
  useImperativeHandle,
} from 'react';
import { useTranslation } from 'react-i18next';
import { WebTerminal } from './WebTerminal';
import type { WebTerminalHandle } from './WebTerminal';
import { ShortcutBar } from './ShortcutBar';
import { executeServerCommand } from '@/lib/server-commands';
import type { Server } from '@shared';
import type { ConnectionState } from '@/lib/types';
import type { SessionWithState } from '@/lib/types/session';

interface SplitPane {
  id: string;
  sessionId?: string;
  windowName?: string;
  isCreating?: boolean;
}

export interface SplitTerminalHandle {
  addPane: (direction: 'horizontal' | 'vertical') => void;
  closePane: () => void;
  selectSessionInFocusedPane: (sessionId: string) => Promise<void>;
  getPaneCount: () => number;
  resetToSinglePane: () => void;
}

interface SplitTerminalProps {
  server: Server;
  repoPath: string;
  featureName: string;
  sessionId?: string;
  worktreePath: string;
  shellSessions?: SessionWithState[];
  dialogOpen?: boolean;
  onConnectionStateChange?: (state: ConnectionState) => void;
  onToggleDiff?: () => void;
}

type TwoPaneLayout = 'horizontal' | 'vertical';

const AREA_NAMES = ['a', 'b', 'c', 'd'];

function generatePaneId(): string {
  if (typeof crypto !== 'undefined' && crypto.randomUUID) {
    return crypto.randomUUID();
  }
  return Date.now().toString(36) + Math.random().toString(36).slice(2);
}

function getMaxPanesForWidth(width: number): number {
  if (width < 1024) return 1;
  if (width < 1280) return 2;
  return 4;
}

/** Responsive max pane count based on viewport width (UX spec breakpoints). */
function useMaxPanes(): number {
  const [maxPanes, setMaxPanes] = useState(() => {
    if (typeof window === 'undefined') return 4;
    return getMaxPanesForWidth(window.innerWidth);
  });

  useEffect(() => {
    const handleResize = () => {
      setMaxPanes(getMaxPanesForWidth(window.innerWidth));
    };
    window.addEventListener('resize', handleResize);
    return () => window.removeEventListener('resize', handleResize);
  }, []);

  return maxPanes;
}

function getGridStyle(paneCount: number, twoPaneLayout: TwoPaneLayout): React.CSSProperties {
  switch (paneCount) {
    case 1:
      return {
        gridTemplateAreas: '"a"',
        gridTemplateRows: '1fr',
        gridTemplateColumns: '1fr',
      };
    case 2:
      return twoPaneLayout === 'horizontal'
        ? {
            gridTemplateAreas: '"a b"',
            gridTemplateRows: '1fr',
            gridTemplateColumns: '1fr 1fr',
          }
        : {
            gridTemplateAreas: '"a" "b"',
            gridTemplateRows: '1fr 1fr',
            gridTemplateColumns: '1fr',
          };
    case 3:
      return {
        gridTemplateAreas: '"a b" "a c"',
        gridTemplateRows: '1fr 1fr',
        gridTemplateColumns: '1fr 1fr',
      };
    case 4:
      return {
        gridTemplateAreas: '"a b" "c d"',
        gridTemplateRows: '1fr 1fr',
        gridTemplateColumns: '1fr 1fr',
      };
    default:
      return {};
  }
}

/** Close a shell session via the server API (fire-and-forget). */
function closeSessionApi(server: Server, sessionId: string) {
  executeServerCommand(server, {
    action: 'close-session',
    params: { sessionId },
  }).catch((err) => {
    console.warn('[SplitTerminal] close-session failed:', sessionId, err);
  });
}

export const SplitTerminal = forwardRef<SplitTerminalHandle, SplitTerminalProps>(
  ({ server, repoPath, featureName, sessionId, worktreePath, shellSessions, dialogOpen, onConnectionStateChange, onToggleDiff }, ref) => {
    const { t } = useTranslation();
    const maxPanes = useMaxPanes();
    const maxPanesRef = useRef(maxPanes);
    useEffect(() => {
      maxPanesRef.current = maxPanes;
    }, [maxPanes]);

    const [panes, setPanes] = useState<SplitPane[]>(() => [
      { id: generatePaneId(), sessionId, windowName: featureName },
    ]);
    const [focusedPaneId, setFocusedPaneId] = useState('');
    const [twoPaneLayout, setTwoPaneLayout] = useState<TwoPaneLayout>('horizontal');

    // Keep server ref for async closures
    const serverRef = useRef(server);
    useEffect(() => { serverRef.current = server; }, [server]);

    const worktreePathRef = useRef(worktreePath);
    useEffect(() => { worktreePathRef.current = worktreePath; }, [worktreePath]);

    // Resolve effective focus — handles empty initial state and closed panes
    const effectiveFocusId =
      panes.find((p) => p.id === focusedPaneId)?.id ?? panes[0]?.id ?? '';
    const effectiveFocusIdRef = useRef(effectiveFocusId);
    useEffect(() => {
      effectiveFocusIdRef.current = effectiveFocusId;
    }, [effectiveFocusId]);

    // Track pane count for keyboard handler (avoids stale closure)
    const paneCountRef = useRef(panes.length);
    useEffect(() => {
      paneCountRef.current = panes.length;
    }, [panes.length]);

    // Track dialog state for keyboard handler (avoids stale closure)
    const dialogOpenRef = useRef(dialogOpen ?? false);
    useEffect(() => {
      dialogOpenRef.current = dialogOpen ?? false;
    }, [dialogOpen]);

    // Collapse excess panes when viewport shrinks — close sessions for removed panes
    useEffect(() => {
      setPanes((prev) => {
        if (prev.length <= maxPanes) return prev;
        const removed = prev.slice(maxPanes);
        for (const p of removed) {
          if (p.sessionId) closeSessionApi(serverRef.current, p.sessionId);
        }
        return prev.slice(0, maxPanes);
      });
    }, [maxPanes]);

    // Restore shell panes from shellSessions at mount (stateless recovery)
    const initializedForWorktreeRef = useRef('');
    useEffect(() => {
      if (initializedForWorktreeRef.current === worktreePath) return;
      if (!shellSessions || shellSessions.length === 0) return;
      initializedForWorktreeRef.current = worktreePath;

      const max = maxPanesRef.current;
      const toRestore = shellSessions.slice(0, max - 1); // reserve slot 0 for feature pane
      if (toRestore.length === 0) return;

      setPanes((prev) => {
        const newPanes = [...prev];
        for (const session of toRestore) {
          if (newPanes.length >= max) break;
          // Skip if we already have a pane for this session
          if (newPanes.some((p) => p.sessionId === session.sessionId)) continue;
          newPanes.push({
            id: generatePaneId(),
            sessionId: session.sessionId,
          });
        }
        return newPanes;
      });
    }, [shellSessions, worktreePath]);

    // Map of pane ID -> WebTerminalHandle for imperative delegation
    const paneRefsMap = useRef<Map<string, WebTerminalHandle | null>>(new Map());

    // Per-pane connection states for aggregation
    const paneConnectionStates = useRef<Map<string, ConnectionState>>(new Map());
    const onConnectionStateChangeRef = useRef(onConnectionStateChange);
    useEffect(() => {
      onConnectionStateChangeRef.current = onConnectionStateChange;
    }, [onConnectionStateChange]);

    const aggregateAndForward = useCallback(() => {
      const states = Array.from(paneConnectionStates.current.values());
      if (states.length === 0) return;

      let aggregated: ConnectionState;
      if (states.some((s) => s.status === 'connected')) {
        aggregated = { status: 'connected', reconnectAttempts: 0 };
      } else if (states.some((s) => s.status === 'reconnecting')) {
        const r = states.find((s) => s.status === 'reconnecting')!;
        aggregated = { status: 'reconnecting', reconnectAttempts: r.reconnectAttempts };
      } else if (states.some((s) => s.status === 'connecting')) {
        aggregated = { status: 'connecting', reconnectAttempts: 0 };
      } else {
        const err = states.find((s) => s.status === 'error');
        aggregated = err || { status: 'disconnected', reconnectAttempts: 0 };
      }
      onConnectionStateChangeRef.current?.(aggregated);
    }, []);

    const handlePaneConnectionStateChange = useCallback(
      (paneId: string, state: ConnectionState) => {
        paneConnectionStates.current.set(paneId, state);
        aggregateAndForward();
      },
      [aggregateAndForward]
    );

    // Track whether an addPane is in-flight (prevent double-fire from keyboard repeat)
    const addingPaneRef = useRef(false);

    const addPane = useCallback((direction: 'horizontal' | 'vertical') => {
      if (addingPaneRef.current) return;
      if (paneCountRef.current >= maxPanesRef.current) return;

      const placeholderId = generatePaneId();
      addingPaneRef.current = true;

      // 1. Add placeholder immediately (reactive UI)
      setPanes((prev) => {
        if (prev.length >= maxPanesRef.current) return prev;
        if (prev.length === 1) {
          setTwoPaneLayout(direction);
        }
        return [...prev, { id: placeholderId, isCreating: true }];
      });

      // 2. Create shell session via API
      executeServerCommand(serverRef.current, {
        action: 'create-session',
        params: { worktreePath: worktreePathRef.current, agentType: 'shell' },
      })
        .then((result) => {
          const sid = result.data?.session?.sessionId ?? result.data?.sessionId;
          if (result.success && sid) {
            setPanes((prev) =>
              prev.map((p) =>
                p.id === placeholderId
                  ? { ...p, sessionId: sid, isCreating: false }
                  : p
              )
            );
          } else {
            // Failed — remove placeholder
            console.warn('[SplitTerminal] create-session failed:', result.error);
            setPanes((prev) => prev.filter((p) => p.id !== placeholderId));
          }
        })
        .catch((err) => {
          console.warn('[SplitTerminal] create-session error:', err);
          setPanes((prev) => prev.filter((p) => p.id !== placeholderId));
        })
        .finally(() => {
          addingPaneRef.current = false;
        });
    }, []);

    const closePane = useCallback(() => {
      const focusedId = effectiveFocusIdRef.current;
      let didClose = false;

      setPanes((prev) => {
        if (prev.length <= 1) return prev;
        // Don't allow closing the first pane (feature window)
        if (prev[0].id === focusedId) return prev;
        const closing = prev.find((p) => p.id === focusedId);
        if (closing?.sessionId) {
          closeSessionApi(serverRef.current, closing.sessionId);
        }
        didClose = true;
        return prev.filter((p) => p.id !== focusedId);
      });

      if (didClose) {
        paneConnectionStates.current.delete(focusedId);
        setFocusedPaneId('');
        aggregateAndForward();
      }
    }, [aggregateAndForward]);

    const resetToSinglePane = useCallback(() => {
      setPanes((prev) => {
        if (prev.length <= 1) return prev;
        const keepId = prev[0].id;
        // Close sessions for removed panes
        for (const p of prev.slice(1)) {
          if (p.sessionId) closeSessionApi(serverRef.current, p.sessionId);
        }
        for (const id of paneConnectionStates.current.keys()) {
          if (id !== keepId) paneConnectionStates.current.delete(id);
        }
        return [prev[0]];
      });
      setFocusedPaneId('');
      aggregateAndForward();
    }, [aggregateAndForward]);

    const selectSessionInFocusedPane = useCallback(async (sid: string) => {
      const focusedId = effectiveFocusIdRef.current;
      setPanes((prev) =>
        prev.map((p) => (p.id === focusedId ? { ...p, sessionId: sid } : p))
      );
      const handle = paneRefsMap.current.get(focusedId);
      if (handle) {
        await handle.selectSession(sid);
      }
    }, []);

    /** Route ShortcutBar input to the focused pane's terminal. */
    const sendToFocusedPane = useCallback((data: string) => {
      const focusedId = effectiveFocusIdRef.current;
      const handle = paneRefsMap.current.get(focusedId);
      if (handle) {
        handle.sendInput(data);
      }
    }, []);

    useImperativeHandle(
      ref,
      () => ({
        addPane,
        closePane,
        selectSessionInFocusedPane,
        getPaneCount: () => panes.length,
        resetToSinglePane,
      }),
      [addPane, closePane, selectSessionInFocusedPane, panes.length, resetToSinglePane]
    );

    // Keyboard shortcuts: Cmd+D (split H), Cmd+Shift+D (split V), Cmd+W (close pane)
    useEffect(() => {
      const handleKeyDown = (e: KeyboardEvent) => {
        if (dialogOpenRef.current) return;
        if (!e.metaKey) return;
        const key = e.key.toLowerCase();
        if (key === 'd' && !e.shiftKey) {
          e.preventDefault();
          addPane('horizontal');
        } else if (key === 'd' && e.shiftKey) {
          e.preventDefault();
          addPane('vertical');
        } else if (key === 'w' && paneCountRef.current > 1) {
          e.preventDefault();
          closePane();
        }
      };
      window.addEventListener('keydown', handleKeyDown);
      return () => window.removeEventListener('keydown', handleKeyDown);
    }, [addPane, closePane]);

    const isMultiPane = panes.length > 1;
    const gridStyle = getGridStyle(panes.length, twoPaneLayout);

    return (
      <div className="flex flex-col flex-1 overflow-hidden">
        <div
          role="group"
          aria-label={t('terminal.split_panes')}
          className="grid flex-1 overflow-hidden gap-px"
          style={gridStyle}>
          {panes.map((pane, index) => {
            const isFocused = pane.id === effectiveFocusId;
            return (
              <div
                key={pane.id}
                role="region"
                aria-label={`${t('terminal.pane_label', { index: index + 1 })}${isFocused ? ' ' + t('terminal.pane_focused') : ''}`}
                onClick={() => setFocusedPaneId(pane.id)}
                className={`flex flex-col overflow-hidden rounded-sm ${
                  isFocused
                    ? 'border border-primary'
                    : 'border border-border'
                }`}
                style={{ gridArea: AREA_NAMES[index] }}>
                {pane.isCreating ? (
                  <div className="flex-1 flex items-center justify-center bg-background text-muted-foreground text-[13px]">
                    {t('terminal.creating_session')}
                  </div>
                ) : (
                  <WebTerminal
                    ref={(handle) => {
                      if (handle) {
                        paneRefsMap.current.set(pane.id, handle);
                      } else {
                        paneRefsMap.current.delete(pane.id);
                      }
                    }}
                    server={server}
                    repoPath={repoPath}
                    featureName={pane.windowName || featureName}
                    sessionId={pane.sessionId}
                    hideShortcutBar={isMultiPane}
                    onConnectionStateChange={(state) =>
                      handlePaneConnectionStateChange(pane.id, state)
                    }
                    onToggleDiff={onToggleDiff}
                  />
                )}
              </div>
            );
          })}
        </div>
        {isMultiPane && <ShortcutBar onSend={sendToFocusedPane} onToggleDiff={onToggleDiff} />}
      </div>
    );
  }
);

SplitTerminal.displayName = 'SplitTerminal';
