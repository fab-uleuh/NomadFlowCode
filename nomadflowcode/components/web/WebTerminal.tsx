import { useEffect, useRef, useState, useCallback, forwardRef, useImperativeHandle } from 'react';
import { useTranslation } from 'react-i18next';
import { useStorage } from '@/lib/context/storage-context';
import { useTerminalWs } from '@/lib/hooks/useTerminalWs';
import type { PaneCallbacks } from '@/lib/hooks/useTerminalWs';
import { ShortcutBar } from './ShortcutBar';
import type { PaneInfoDto } from '@/lib/terminal-ws';

export interface WebTerminalHandle {
  selectSession: (sessionId: string) => Promise<void>;
  sendInput: (data: string) => void;
  paneId?: number;
}

/** Extract repo name from a full path. */
function repoName(repoPath: string): string {
  return repoPath.split('/').filter(Boolean).pop() || repoPath;
}

interface WebTerminalProps {
  repoPath: string;
  featureName: string;
  worktreePath?: string;
  agentType?: string;
  sessionId?: string;
  onReady?: () => void;
  hideShortcutBar?: boolean;
  onConnectionStateChange?: (state: import('@/lib/types').ConnectionState) => void;
  onToggleDiff?: () => void;
  /** Called when a subscribed pane is destroyed server-side (allows parent to update UI). */
  onPaneDestroyed?: () => void;
  /** If true, destroyPane on unmount (used when closing a pane, not just switching). */
  destroyOnUnmount?: boolean;
}

export const WebTerminal = forwardRef<WebTerminalHandle, WebTerminalProps>(
  ({ repoPath, featureName, worktreePath, agentType = 'claude', sessionId: _sessionIdProp, onReady, hideShortcutBar, onConnectionStateChange, onToggleDiff, onPaneDestroyed, destroyOnUnmount }, ref) => {
    const { t } = useTranslation();
    const { settings } = useStorage();
    const ws = useTerminalWs();
    const terminalRef = useRef<HTMLDivElement>(null);
    const termRef = useRef<any>(null);
    const fitAddonRef = useRef<any>(null);
    const activePaneIdRef = useRef<number | undefined>(undefined);
    const tRef = useRef(t);
    const onReadyRef = useRef(onReady);
    const onConnectionStateChangeRef = useRef(onConnectionStateChange);
    const repoPathRef = useRef(repoPath);
    const featureNameRef = useRef(featureName);
    const worktreePathRef = useRef(worktreePath || repoPath);
    const agentTypeRef = useRef(agentType);
    const onPaneDestroyedRef = useRef(onPaneDestroyed);
    const destroyOnUnmountRef = useRef(destroyOnUnmount ?? false);
    const settingsRef = useRef(settings);
    const [termReady, setTermReady] = useState(false);
    const initialPaneCreatedRef = useRef(false);
    const prevConnectionStatusRef = useRef(ws.connectionState.status);
    const resetPendingRef = useRef(false);

    useEffect(() => { tRef.current = t; }, [t]);
    useEffect(() => { settingsRef.current = settings; }, [settings]);
    useEffect(() => { onReadyRef.current = onReady; }, [onReady]);
    useEffect(() => { onConnectionStateChangeRef.current = onConnectionStateChange; }, [onConnectionStateChange]);
    useEffect(() => { repoPathRef.current = repoPath; }, [repoPath]);
    useEffect(() => { featureNameRef.current = featureName; }, [featureName]);
    useEffect(() => { worktreePathRef.current = worktreePath || repoPath; }, [worktreePath, repoPath]);
    useEffect(() => { agentTypeRef.current = agentType; }, [agentType]);
    useEffect(() => { onPaneDestroyedRef.current = onPaneDestroyed; }, [onPaneDestroyed]);
    useEffect(() => { destroyOnUnmountRef.current = destroyOnUnmount ?? false; }, [destroyOnUnmount]);

    // Forward WS connection state to parent
    useEffect(() => {
      onConnectionStateChangeRef.current?.(ws.connectionState);
    }, [ws.connectionState]);

    // Reset xterm on reconnection to prevent display corruption (AC #3)
    // Fires before the provider's re-subscribe effect (child effects run first),
    // so terminal.reset() happens before the server sends BufferSnapshot.
    useEffect(() => {
      const prevStatus = prevConnectionStatusRef.current;
      const currentStatus = ws.connectionState.status;
      prevConnectionStatusRef.current = currentStatus;

      if (prevStatus === 'reconnecting' && currentStatus === 'connected') {
        const terminal = termRef.current;
        if (terminal) {
          terminal.reset();
          resetPendingRef.current = true;
          // Clear reset flag after a microtask to ensure snapshot processing sees it
          Promise.resolve().then(() => {
            resetPendingRef.current = false;
          });
        }
      }
    }, [ws.connectionState.status]);

    const sendToTerminal = useCallback((data: string) => {
      const paneId = activePaneIdRef.current;
      if (paneId != null) {
        ws.sendInput(paneId, data);
      }
    }, [ws]);

    const sendResize = useCallback((cols: number, rows: number) => {
      const paneId = activePaneIdRef.current;
      if (paneId != null) {
        ws.sendResize(paneId, cols, rows);
      }
    }, [ws]);

    // Expose selectSession, sendInput, paneId for parent
    useImperativeHandle(
      ref,
      () => ({
        selectSession: async (sessionId: string) => {
          // Find pane by numeric ID or by label
          const numericId = Number(sessionId);
          const targetPane = !isNaN(numericId)
            ? ws.paneList.find((p) => p.id === numericId)
            : ws.paneList.find((p) => p.label === sessionId);
          if (!targetPane) {
            console.warn('[WebTerminal] No pane found for sessionId:', sessionId);
            return;
          }

          const currentPaneId = activePaneIdRef.current;
          if (currentPaneId === targetPane.id) return; // already on this pane

          const terminal = termRef.current;
          if (!terminal) return; // can't switch without xterm initialized (M1 fix)

          // Update activePaneIdRef synchronously before any async operation (Task 2.4)
          activePaneIdRef.current = targetPane.id;

          // Clear old content for clean transition (Task 2.2)
          terminal.clear();
          // Atomic switch: unsubscribe old + subscribe new (Task 2.1)
          ws.switchPane(currentPaneId, targetPane.id, makePaneCallbacks(terminal));
        },
        sendInput: (data: string) => {
          sendToTerminal(data);
        },
        get paneId() {
          return activePaneIdRef.current;
        },
      }),
      [ws, sendToTerminal]
    );

    /** Build pane-specific callbacks that route output to this component's xterm instance. */
    function makePaneCallbacks(terminal: any): PaneCallbacks {
      return {
        onPtyData: (data: Uint8Array) => {
          terminal.write(data);
        },
        onBufferSnapshot: (data: Uint8Array) => {
          // Skip snapshot if reset is pending (race condition fix)
          if (resetPendingRef.current) {
            Promise.resolve().then(() => {
              if (!resetPendingRef.current) {
                terminal.write(data);
              }
            });
            return;
          }
          terminal.write(data);
        },
        onDestroyed: () => {
          activePaneIdRef.current = undefined;
          // Show inline message instead of blank screen (Task 4.1, 4.2)
          terminal.write('\r\n\x1b[1;33m[Terminal session ended]\x1b[0m\r\n');
          // Notify parent so it can update UI (e.g., remove tab) (M3 fix)
          onPaneDestroyedRef.current?.();
        },
      };
    }

    // Main init effect: set up xterm.js and create/subscribe pane
    useEffect(() => {
      let cancelled = false;

      async function init() {
        const { Terminal } = await import('@xterm/xterm');
        const { FitAddon } = await import('@xterm/addon-fit');
        const { WebLinksAddon } = await import('@xterm/addon-web-links');
        await import('@xterm/xterm/css/xterm.css');

        if (cancelled || !terminalRef.current) return;

        const term = new Terminal({
          fontSize: Math.min(settings.fontSize, 24),
          fontFamily: "'JetBrains Mono', 'Fira Code', 'Cascadia Code', Menlo, Monaco, monospace",
          cursorBlink: true,
          theme: {
            background: '#0e0e12',
            foreground: '#e4e4ec',
            cursor: '#e4e4ec',
            selectionBackground: '#3d3d5c',
          },
          allowProposedApi: true,
        });

        const fitAddon = new FitAddon();
        const webLinksAddon = new WebLinksAddon();
        term.loadAddon(fitAddon);
        term.loadAddon(webLinksAddon);

        termRef.current = term;
        fitAddonRef.current = fitAddon;

        term.open(terminalRef.current);
        requestAnimationFrame(() => {
          try { fitAddon.fit(); } catch {}
        });

        term.onData((data: string) => {
          sendToTerminal(data);
        });

        const resizeObserver = new ResizeObserver(() => {
          try {
            fitAddon.fit();
            if (term.cols && term.rows) {
              sendResize(term.cols, term.rows);
            }
          } catch {}
        });

        if (terminalRef.current) {
          resizeObserver.observe(terminalRef.current);
        }
        (term as any)._resizeObserver = resizeObserver;

        // Signal xterm ready — pane creation handled by the combined effect below
        setTermReady(true);
      }

      init();

      return () => {
        cancelled = true;
        setTermReady(false);
        initialPaneCreatedRef.current = false;
        const paneId = activePaneIdRef.current;
        if (paneId != null) {
          if (destroyOnUnmountRef.current) {
            ws.destroyPane(paneId);
          } else {
            ws.unsubscribe(paneId);
          }
        }
        activePaneIdRef.current = undefined;
        const t = termRef.current;
        if (t) {
          (t as any)._resizeObserver?.disconnect();
          t.dispose();
        }
        termRef.current = null;
        fitAddonRef.current = null;
      };
    }, []); // Stable — uses refs for all mutable state

    // Create pane when WS is connected, xterm is ready, AND pane list has arrived.
    // Handles initial connection, reconnection, and any ordering of the events.
    useEffect(() => {
      if (ws.connectionState.status !== 'connected') return;
      if (!termReady) return;
      if (!ws.paneListReady) return;
      const terminal = termRef.current;
      if (!terminal) return;
      if (activePaneIdRef.current != null) return; // already have a pane

      let cancelled = false;
      (async () => {
        try {
          // Before creating, check if an existing pane matches
          const existingPane = ws.paneList.find(
            (p) => p.repo === repoName(repoPathRef.current)
              && p.worktree === featureNameRef.current
              && p.agentType === agentTypeRef.current
          );

          if (existingPane) {
            if (cancelled) return;
            activePaneIdRef.current = existingPane.id;
            ws.subscribe(existingPane.id, makePaneCallbacks(terminal));

            // Fit and resize after subscribe
            try {
              fitAddonRef.current?.fit();
              if (terminal.cols && terminal.rows) {
                ws.sendResize(existingPane.id, terminal.cols, terminal.rows);
              }
            } catch {}

            initialPaneCreatedRef.current = true;
            onReadyRef.current?.();
            return;
          }

          // No existing pane — create a new one
          const cols = terminal.cols || 80;
          const rows = terminal.rows || 24;
          const info = await ws.createPane({
            repo: repoName(repoPathRef.current),
            worktree: featureNameRef.current,
            agentType: agentTypeRef.current,
            cwd: worktreePathRef.current,
            cols,
            rows,
          });
          if (cancelled) return;

          activePaneIdRef.current = info.id;
          ws.subscribe(info.id, makePaneCallbacks(terminal));

          // Fit and resize after subscribe
          try {
            fitAddonRef.current?.fit();
            if (terminal.cols && terminal.rows) {
              ws.sendResize(info.id, terminal.cols, terminal.rows);
            }
          } catch {}

          // Auto-launch agent only on first pane creation (not reconnection)
          if (!initialPaneCreatedRef.current) {
            initialPaneCreatedRef.current = true;
            const s = settingsRef.current;
            setTimeout(() => {
              if (cancelled) return;
              if (s.autoLaunchAgent && agentTypeRef.current !== 'shell') {
                const agentCommand =
                  s.defaultAiAgent === 'claude'
                    ? 'claude'
                    : s.defaultAiAgent === 'ollama'
                      ? 'ollama run deepseek-coder'
                      : s.customAgentCommand || `echo "${tRef.current('terminal.error.no_agent')}"`;
                ws.sendInput(info.id, agentCommand + '\n');
              }
            }, 500);
          }

          onReadyRef.current?.();
        } catch (err) {
          console.error('[WebTerminal] Failed to create pane:', err);
        }
      })();

      return () => { cancelled = true; };
    }, [ws.connectionState.status, termReady, ws.paneListReady]);

    const statusColorClass =
      ws.connectionState.status === 'connected'
        ? 'bg-success'
        : ws.connectionState.status === 'error' || ws.connectionState.status === 'disconnected'
          ? 'bg-destructive'
          : 'bg-warning';

    const statusTextClass =
      ws.connectionState.status === 'connected'
        ? 'text-success'
        : ws.connectionState.status === 'error' || ws.connectionState.status === 'disconnected'
          ? 'text-destructive'
          : 'text-warning';

    const statusLabel =
      ws.connectionState.status === 'reconnecting'
        ? t('terminal.status.reconnecting_count', { attempts: ws.connectionState.reconnectAttempts, max: settings.maxReconnectAttempts })
        : t(`terminal.status.${ws.connectionState.status}`);

    return (
      <div className="flex flex-col flex-1 overflow-hidden">
        {/* Terminal header */}
        <div className="flex items-center px-3 py-1.5 border-b border-border bg-card gap-2 shrink-0">
          <span className="font-semibold text-[13px]">{featureName}</span>
          <span className={`w-2 h-2 rounded-full ${statusColorClass}`} />
          <span className={`text-xs ${statusTextClass}`}>{statusLabel}</span>
          {ws.connectionState.error && (
            <span className="text-[11px] text-destructive">{ws.connectionState.error}</span>
          )}
          <span className="flex-1" />
        </div>

        {/* Terminal container */}
        <div
          ref={terminalRef}
          className="flex-1 bg-[#0e0e12] p-1 overflow-hidden"
        />

        {/* Shortcut bar (hidden when inside multi-pane SplitTerminal) */}
        {!hideShortcutBar && <ShortcutBar onSend={sendToTerminal} onToggleDiff={onToggleDiff} />}
      </div>
    );
  }
);

WebTerminal.displayName = 'WebTerminal';
