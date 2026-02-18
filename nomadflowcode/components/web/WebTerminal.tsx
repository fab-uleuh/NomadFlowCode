import { useEffect, useRef, useState, useCallback, forwardRef, useImperativeHandle } from 'react';
import { useTranslation } from 'react-i18next';
import { switchToFeature, executeServerCommand } from '@/lib/server-commands';
import { useStorage } from '@/lib/context/storage-context';
import { ShortcutBar } from './ShortcutBar';
import type { Server } from '@shared';
import type { ConnectionState } from '@/lib/types';

export interface WebTerminalHandle {
  selectSession: (sessionId: string) => Promise<void>;
  sendInput: (data: string) => void;
}

interface WebTerminalProps {
  server: Server;
  repoPath: string;
  featureName: string;
  sessionId?: string;
  onReady?: () => void;
  hideShortcutBar?: boolean;
  onConnectionStateChange?: (state: ConnectionState) => void;
  onToggleDiff?: () => void;
}

/** Derive the WS URL from the server config or current page origin. */
function buildWsUrl(server: Server): string {
  // If apiUrl matches current origin or is empty, use page origin (same-origin WS)
  const origin =
    typeof window !== 'undefined' ? window.location.origin : 'http://localhost:8080';
  let baseUrl = (server.apiUrl || origin).replace(/\/+$/, '');

  // Strip /api suffix if present (WS route is /terminal/ws, not /api/terminal/ws)
  baseUrl = baseUrl.replace(/\/api$/, '');

  const wsScheme = baseUrl.startsWith('https') ? 'wss' : 'ws';
  const host = baseUrl.replace(/^https?:\/\//, '');
  let wsUrl = `${wsScheme}://${host}/terminal/ws`;
  if (server.authToken) {
    wsUrl += `?token=${encodeURIComponent(server.authToken)}`;
  }
  return wsUrl;
}

/** Send a command through the WS using ttyd binary protocol (0x30 = input). */
function sendWsInput(ws: WebSocket, data: string) {
  const encoder = new TextEncoder();
  const encoded = encoder.encode(data);
  const bytes = new Uint8Array(encoded.length + 1);
  bytes[0] = 0x30; // '0' = input
  bytes.set(encoded, 1);
  ws.send(bytes);
}

/** Call the server API to select a tmux window by session ID. */
async function selectSessionViaApi(
  server: Server,
  sessionId: string,
  linkedSession?: string | null
): Promise<boolean> {
  try {
    const params: Record<string, string> = { sessionId };
    if (linkedSession) params.linkedSession = linkedSession;
    const result = await executeServerCommand(server, {
      action: 'select-session',
      params,
    });
    if (result.success) {
      console.log('[WebTerminal] select-session OK for:', sessionId);
      return true;
    }
    console.warn('[WebTerminal] select-session failed:', result.error);
    return false;
  } catch (err) {
    console.warn('[WebTerminal] select-session error:', err);
    return false;
  }
}

export const WebTerminal = forwardRef<WebTerminalHandle, WebTerminalProps>(
  ({ server, repoPath, featureName, sessionId: sessionIdProp, onReady, hideShortcutBar, onConnectionStateChange, onToggleDiff }, ref) => {
    const { t } = useTranslation();
    const { settings, updateServer } = useStorage();
    const terminalRef = useRef<HTMLDivElement>(null);
    const termRef = useRef<any>(null);
    const wsRef = useRef<WebSocket | null>(null);
    const fitAddonRef = useRef<any>(null);
    const reconnectTimerRef = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
    const reconnectAttemptsRef = useRef(0);
    const hasRunningProcessRef = useRef(false);
    const wsClosedIntentionallyRef = useRef(false);
    const sessionIdRef = useRef<string | undefined>(sessionIdProp);
    const linkedSessionRef = useRef<string | null>(null);
    const tRef = useRef(t);
    const onReadyRef = useRef(onReady);
    const onConnectionStateChangeRef = useRef(onConnectionStateChange);
    const repoPathRef = useRef(repoPath);
    const featureNameRef = useRef(featureName);

    // Keep refs in sync with props — but do NOT trigger re-init
    useEffect(() => {
      sessionIdRef.current = sessionIdProp;
    }, [sessionIdProp]);

    useEffect(() => {
      tRef.current = t;
    }, [t]);

    useEffect(() => {
      onReadyRef.current = onReady;
    }, [onReady]);

    useEffect(() => {
      onConnectionStateChangeRef.current = onConnectionStateChange;
    }, [onConnectionStateChange]);

    useEffect(() => {
      repoPathRef.current = repoPath;
    }, [repoPath]);

    useEffect(() => {
      featureNameRef.current = featureName;
    }, [featureName]);

    const [connectionState, setConnectionState] = useState<ConnectionState>({
      status: 'connecting',
      reconnectAttempts: 0,
    });

    // Notify parent of connection state changes
    useEffect(() => {
      onConnectionStateChangeRef.current?.(connectionState);
    }, [connectionState]);

    const sendToTerminal = useCallback((data: string) => {
      const ws = wsRef.current;
      if (ws && ws.readyState === WebSocket.OPEN) {
        sendWsInput(ws, data);
      }
    }, []);

    const sendResize = useCallback((cols: number, rows: number) => {
      const ws = wsRef.current;
      if (ws && ws.readyState === WebSocket.OPEN) {
        const json = JSON.stringify({ columns: cols, rows });
        const bytes = new Uint8Array(json.length + 1);
        bytes[0] = 0x31; // '1' = resize
        for (let i = 0; i < json.length; i++) {
          bytes[i + 1] = json.charCodeAt(i);
        }
        ws.send(bytes.buffer);
      }
    }, []);

    // Expose selectSession and sendInput for parent to call (imperative handle)
    useImperativeHandle(
      ref,
      () => ({
        selectSession: async (sessionId: string) => {
          sessionIdRef.current = sessionId;
          await selectSessionViaApi(server, sessionId, linkedSessionRef.current);
        },
        sendInput: (data: string) => {
          sendToTerminal(data);
        },
      }),
      [server, sendToTerminal]
    );

    /** Handle incoming WS messages: text = server metadata, binary = ttyd protocol. */
    function handleWsMessage(terminal: any, event: MessageEvent) {
      // Text message = server metadata (linked session name, pushed before first binary output)
      if (typeof event.data === 'string') {
        try {
          const meta = JSON.parse(event.data);
          if (meta.linkedSession) {
            linkedSessionRef.current = meta.linkedSession;
            console.log('[WebTerminal] Linked session:', meta.linkedSession);
            // Re-select the correct window in the linked session
            if (sessionIdRef.current) {
              selectSessionViaApi(server, sessionIdRef.current, meta.linkedSession);
            } else {
              switchToFeature(server, {
                repoPath: repoPathRef.current,
                featureName: featureNameRef.current,
                linkedSession: meta.linkedSession,
              });
            }
          }
        } catch {}
        return;
      }
      // Binary message = ttyd protocol
      const data = new Uint8Array(event.data as ArrayBuffer);
      if (data.length < 1) return;
      const cmd = data[0];
      if (cmd === 0x30) {
        // '0' = output
        terminal.write(data.slice(1));
      }
      // cmd === 0x31 -> window title (ignore)
    }

    // Connect terminal — deps do NOT include tmuxWindow, repoPath, featureName
    useEffect(() => {
      let cancelled = false;

      async function init() {
        // 1. Switch to the right tmux window via server API
        if (sessionIdRef.current) {
          // Session-based: call select-session API (linked session may not be known yet at init)
          await selectSessionViaApi(server, sessionIdRef.current, linkedSessionRef.current);
        } else {
          // Feature-based: call switch-feature API to select the right window
          try {
            console.log(
              '[WebTerminal] Switching to feature:',
              featureNameRef.current,
              'repo:',
              repoPathRef.current
            );
            const result = await switchToFeature(server, {
              repoPath: repoPathRef.current,
              featureName: featureNameRef.current,
              linkedSession: linkedSessionRef.current || undefined,
            });
            if (result.success && result.data) {
              hasRunningProcessRef.current = !!result.data.hasRunningProcess;
              console.log(
                '[WebTerminal] Switch OK, hasRunningProcess:',
                result.data.hasRunningProcess
              );
            } else {
              console.warn('[WebTerminal] Switch failed:', result.error);
            }
          } catch (err) {
            console.warn('[WebTerminal] Error switching feature:', err);
          }
        }

        if (cancelled) return;

        // 2. Dynamically import xterm (web-only)
        const { Terminal } = await import('@xterm/xterm');
        const { FitAddon } = await import('@xterm/addon-fit');
        const { WebLinksAddon } = await import('@xterm/addon-web-links');

        // Import CSS
        await import('@xterm/xterm/css/xterm.css');

        if (cancelled || !terminalRef.current) return;

        // 3. Create terminal
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

        // Fit after render
        requestAnimationFrame(() => {
          try {
            fitAddon.fit();
          } catch {}
        });

        // 4. Connect WebSocket
        connectWs(term);

        // 5. Handle terminal input
        term.onData((data: string) => {
          sendToTerminal(data);
        });

        // 6. Handle resize
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

        // Store for cleanup
        (term as any)._resizeObserver = resizeObserver;
      }

      function connectWs(terminal: any) {
        const wsUrl = buildWsUrl(server);
        console.log('[WebTerminal] Connecting WS to:', wsUrl);

        wsClosedIntentionallyRef.current = false;

        const ws = new WebSocket(wsUrl, ['tty']);
        wsRef.current = ws;
        ws.binaryType = 'arraybuffer';

        ws.onopen = () => {
          console.log('[WebTerminal] WS connected');
          reconnectAttemptsRef.current = 0;
          setConnectionState({ status: 'connected', reconnectAttempts: 0 });

          // Fit and send initial size
          try {
            fitAddonRef.current?.fit();
            if (terminal.cols && terminal.rows) {
              sendResize(terminal.cols, terminal.rows);
            }
          } catch {}

          // Session-based: window already selected via API in init()
          if (sessionIdRef.current) {
            // No action needed — tmux window was selected before WS connect
          } else if (!hasRunningProcessRef.current) {
            // Send init commands if no process running (feature-based)
            setTimeout(() => {
              if (settings.autoLaunchAgent) {
                const agentCommand =
                  settings.defaultAiAgent === 'claude'
                    ? 'claude'
                    : settings.defaultAiAgent === 'ollama'
                      ? 'ollama run deepseek-coder'
                      : settings.customAgentCommand || `echo "${tRef.current('terminal.error.no_agent')}"`;
                sendToTerminal(agentCommand + '\n');
              } else {
                sendToTerminal(
                  `echo "\uD83D\uDE80 NomadFlow - ${featureNameRef.current}"\n`
                );
              }
            }, 500);
          }

          // Notify parent that WS is connected and ready
          onReadyRef.current?.();
        };

        ws.onmessage = (event) => handleWsMessage(terminal, event);

        ws.onclose = (event) => {
          console.log(
            '[WebTerminal] WS closed, code:',
            event.code,
            'reason:',
            event.reason,
            'wasClean:',
            event.wasClean
          );

          if (cancelled || wsClosedIntentionallyRef.current) return;

          const attempts = reconnectAttemptsRef.current;
          if (settings.autoReconnect && attempts < settings.maxReconnectAttempts) {
            reconnectAttemptsRef.current = attempts + 1;
            setConnectionState({
              status: 'reconnecting',
              reconnectAttempts: attempts + 1,
            });
            const backoffDelay = Math.min(1000 * Math.pow(2, attempts), 8000);
            console.log(
              `[WebTerminal] Reconnecting (attempt ${attempts + 1}/${settings.maxReconnectAttempts}) in ${backoffDelay}ms`
            );
            reconnectTimerRef.current = setTimeout(() => {
              connectWs(terminal);
            }, backoffDelay);
          } else {
            setConnectionState({
              status: 'error',
              error:
                attempts >= settings.maxReconnectAttempts
                  ? tRef.current('terminal.error.max_reconnect')
                  : tRef.current('terminal.error.connection_closed', { code: event.code }),
              reconnectAttempts: attempts,
            });
          }
        };

        ws.onerror = (event) => {
          console.error('[WebTerminal] WS error:', event);
          // Don't set state here — onclose will fire right after and handle it
        };
      }

      init();

      updateServer(server.id, { lastConnected: Date.now() });

      return () => {
        cancelled = true;
        wsClosedIntentionallyRef.current = true;
        if (reconnectTimerRef.current) {
          clearTimeout(reconnectTimerRef.current);
        }
        wsRef.current?.close();
        const t = termRef.current;
        if (t) {
          (t as any)._resizeObserver?.disconnect();
          t.dispose();
        }
        termRef.current = null;
        fitAddonRef.current = null;
      };
    }, [server.id, server.apiUrl, server.authToken]);

    const handleReconnect = useCallback(() => {
      reconnectAttemptsRef.current = 0;
      setConnectionState({ status: 'connecting', reconnectAttempts: 0 });
      const term = termRef.current;
      if (!term) return;

      wsClosedIntentionallyRef.current = true;
      wsRef.current?.close();
      wsClosedIntentionallyRef.current = false;

      function attemptConnect() {
        const wsUrl = buildWsUrl(server);
        console.log('[WebTerminal] Reconnect attempt to:', wsUrl);

        const ws = new WebSocket(wsUrl, ['tty']);
        wsRef.current = ws;
        ws.binaryType = 'arraybuffer';

        ws.onopen = () => {
          console.log('[WebTerminal] Reconnected');
          reconnectAttemptsRef.current = 0;
          setConnectionState({ status: 'connected', reconnectAttempts: 0 });
          try {
            fitAddonRef.current?.fit();
            if (term.cols && term.rows) {
              sendResize(term.cols, term.rows);
            }
          } catch {}
        };

        ws.onmessage = (event) => handleWsMessage(term, event);

        ws.onclose = (event) => {
          console.log('[WebTerminal] Reconnect WS closed, code:', event.code);
          if (!wsClosedIntentionallyRef.current) {
            const attempts = reconnectAttemptsRef.current;
            if (settings.autoReconnect && attempts < settings.maxReconnectAttempts) {
              reconnectAttemptsRef.current = attempts + 1;
              setConnectionState({
                status: 'reconnecting',
                reconnectAttempts: attempts + 1,
              });
              const backoffDelay = Math.min(1000 * Math.pow(2, attempts), 8000);
              console.log(
                `[WebTerminal] Reconnecting (attempt ${attempts + 1}/${settings.maxReconnectAttempts}) in ${backoffDelay}ms`
              );
              reconnectTimerRef.current = setTimeout(attemptConnect, backoffDelay);
            } else {
              setConnectionState({
                status: 'error',
                error:
                  attempts >= settings.maxReconnectAttempts
                    ? tRef.current('terminal.error.max_reconnect')
                    : tRef.current('terminal.error.connection_closed', { code: event.code }),
                reconnectAttempts: attempts,
              });
            }
          }
        };

        ws.onerror = (event) => {
          console.error('[WebTerminal] Reconnect WS error:', event);
        };
      }

      attemptConnect();
    }, [server, sendResize]);

    const statusColorClass =
      connectionState.status === 'connected'
        ? 'bg-success'
        : connectionState.status === 'error' || connectionState.status === 'disconnected'
          ? 'bg-destructive'
          : 'bg-warning';

    const statusTextClass =
      connectionState.status === 'connected'
        ? 'text-success'
        : connectionState.status === 'error' || connectionState.status === 'disconnected'
          ? 'text-destructive'
          : 'text-warning';

    const statusLabel =
      connectionState.status === 'reconnecting'
        ? t('terminal.status.reconnecting_count', { attempts: connectionState.reconnectAttempts, max: settings.maxReconnectAttempts })
        : t(`terminal.status.${connectionState.status}`);

    return (
      <div className="flex flex-col flex-1 overflow-hidden">
        {/* Terminal header */}
        <div className="flex items-center px-3 py-1.5 border-b border-border bg-card gap-2 shrink-0">
          <span className="font-semibold text-[13px]">{featureName}</span>
          <span className={`w-2 h-2 rounded-full ${statusColorClass}`} />
          <span className={`text-xs ${statusTextClass}`}>{statusLabel}</span>
          {connectionState.error && (
            <span className="text-[11px] text-destructive">{connectionState.error}</span>
          )}
          <span className="flex-1" />
          {(connectionState.status === 'error' || connectionState.status === 'disconnected') && (
            <button
              onClick={handleReconnect}
              className="px-2.5 py-0.5 border border-border rounded-md bg-transparent text-foreground text-xs cursor-pointer">
              {t('terminal.reconnect')}
            </button>
          )}
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
