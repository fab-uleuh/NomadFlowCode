import {
  createContext,
  useContext,
  useRef,
  useEffect,
  useState,
  useCallback,
  useMemo,
  type ReactNode,
} from 'react';
import { createElement } from 'react';
import { TerminalWsHandler } from '@/lib/terminal-ws';
import type {
  ControlMsg,
  PaneInfoDto,
  CreatePaneRequest,
  TerminalWsHandlerCallbacks,
} from '@/lib/terminal-ws';
import type { Server } from '@shared';
import type { ConnectionState } from '@/lib/types';
import { useStorage } from '@/lib/context/storage-context';

// --- Public types ---

export interface PaneCallbacks {
  onPtyData: (data: Uint8Array) => void;
  onBufferSnapshot: (data: Uint8Array) => void;
  onDestroyed: () => void;
}

export interface TerminalWsApi {
  /** Subscribe to a pane's output. Returns unsubscribe function. */
  subscribe: (paneId: number, callbacks: PaneCallbacks) => void;
  /** Unsubscribe from a pane (stops output delivery, does NOT destroy the pane). */
  unsubscribe: (paneId: number) => void;
  /** Atomic pane switch: unsubscribe old, subscribe new, with rapid-switch guard. */
  switchPane: (fromPaneId: number | undefined, toPaneId: number, callbacks: PaneCallbacks) => void;
  /** Send terminal input to a pane. */
  sendInput: (paneId: number, data: string) => void;
  /** Send resize to a pane. */
  sendResize: (paneId: number, cols: number, rows: number) => void;
  /** Create a new pane. Resolves with PaneInfoDto when server responds. */
  createPane: (req: CreatePaneRequest) => Promise<PaneInfoDto>;
  /** Destroy a pane on the server. */
  destroyPane: (paneId: number) => void;
  /** Current WS connection state. */
  connectionState: ConnectionState;
  /** Current pane list from server. */
  paneList: PaneInfoDto[];
  /** True once the first paneList message has been received after (re)connect. */
  paneListReady: boolean;
}

const TerminalWsContext = createContext<TerminalWsApi | null>(null);

export function useTerminalWs(): TerminalWsApi {
  const ctx = useContext(TerminalWsContext);
  if (!ctx) {
    throw new Error('useTerminalWs must be used within a TerminalWsProvider');
  }
  return ctx;
}

// --- Internals ---

/** Derive the WS URL for the multiplexed pane endpoint. */
function buildWsUrl(server: Server): string {
  const origin =
    typeof window !== 'undefined' ? window.location.origin : 'http://localhost:8080';
  let baseUrl = (server.apiUrl || origin).replace(/\/+$/, '');
  baseUrl = baseUrl.replace(/\/api$/, '');
  const wsScheme = baseUrl.startsWith('https') ? 'wss' : 'ws';
  const host = baseUrl.replace(/^https?:\/\//, '');
  return `${wsScheme}://${host}/ws/panes`;
}

interface TerminalWsProviderProps {
  server: Server;
  children: ReactNode;
}

export function TerminalWsProvider({ server, children }: TerminalWsProviderProps) {
  const { settings } = useStorage();

  // Mutable refs for internal state (avoids stale closures)
  const handlerRef = useRef<TerminalWsHandler | null>(null);
  const paneCallbacksRef = useRef<Map<number, PaneCallbacks>>(new Map());
  const pendingCreatesRef = useRef<Array<{ resolve: (info: PaneInfoDto) => void; reject: (err: Error) => void }>>([]);
  const reconnectTimerRef = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  const reconnectAttemptsRef = useRef(0);
  const stableTimerRef = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  const closedIntentionallyRef = useRef(false);
  const settingsRef = useRef(settings);
  useEffect(() => { settingsRef.current = settings; }, [settings]);

  // React state for consumers
  const [connectionState, setConnectionState] = useState<ConnectionState>({
    status: 'connecting',
    reconnectAttempts: 0,
  });
  const [paneList, setPaneList] = useState<PaneInfoDto[]>([]);
  const [paneListReady, setPaneListReady] = useState(false);

  // Stable refs for the current server to use in callbacks
  const serverRef = useRef(server);
  useEffect(() => { serverRef.current = server; }, [server]);

  // --- WS Callback factory ---
  const makeCallbacks = useCallback(
    (handler: TerminalWsHandler, reconnectFn: () => void): TerminalWsHandlerCallbacks => ({
      onOpen: () => {
        setPaneListReady(false);
        setConnectionState({ status: 'connected', reconnectAttempts: 0 });
        // Auto-request pane list on connect (AC #5)
        handler.sendControl({ type: 'list' });
        // Stable-connection timer: reset backoff only after 30s of stable connection (AC #7)
        if (stableTimerRef.current) clearTimeout(stableTimerRef.current);
        stableTimerRef.current = setTimeout(() => {
          reconnectAttemptsRef.current = 0;
        }, 30_000);
      },

      onPtyData: (paneId: number, data: Uint8Array) => {
        paneCallbacksRef.current.get(paneId)?.onPtyData(data);
      },

      onBufferSnapshot: (paneId: number, data: Uint8Array) => {
        paneCallbacksRef.current.get(paneId)?.onBufferSnapshot(data);
      },

      onControl: (msg: ControlMsg) => {
        switch (msg.type) {
          case 'paneCreated': {
            const info = msg as PaneInfoDto & { type: 'paneCreated' };
            // Resolve pending createPane promise
            const pending = pendingCreatesRef.current.shift();
            if (pending) {
              pending.resolve(info);
            }
            // Update pane list
            setPaneList((prev) => {
              if (prev.some((p) => p.id === info.id)) return prev;
              return [...prev, info];
            });
            break;
          }
          case 'paneList': {
            const listMsg = msg as { type: 'paneList'; panes: PaneInfoDto[] };
            setPaneList(listMsg.panes);
            setPaneListReady(true);
            break;
          }
          case 'paneDestroyed': {
            const destroyMsg = msg as { type: 'paneDestroyed'; paneId: number };
            setPaneList((prev) => prev.filter((p) => p.id !== destroyMsg.paneId));
            // Notify subscribed callback
            paneCallbacksRef.current.get(destroyMsg.paneId)?.onDestroyed();
            paneCallbacksRef.current.delete(destroyMsg.paneId);
            break;
          }
          case 'error': {
            const errMsg = msg as { type: 'error'; message: string };
            console.error('[TerminalWsProvider] Server error:', errMsg.message);
            break;
          }
        }
      },

      onClose: (code: number, _reason: string) => {
        if (closedIntentionallyRef.current) return;
        // Cancel stable-connection timer on disconnect
        if (stableTimerRef.current) { clearTimeout(stableTimerRef.current); stableTimerRef.current = undefined; }
        // Do NOT auto-reconnect on normal/policy closure (AC #1)
        if (code === 1000 || code === 1008) {
          setConnectionState({
            status: 'disconnected',
            reconnectAttempts: 0,
          });
          return;
        }

        const s = settingsRef.current;
        const attempts = reconnectAttemptsRef.current;
        if (s.autoReconnect && attempts < s.maxReconnectAttempts) {
          reconnectAttemptsRef.current = attempts + 1;
          setConnectionState({
            status: 'reconnecting',
            reconnectAttempts: attempts + 1,
          });
          // Full Jitter exponential backoff (AC #1)
          const baseDelay = Math.max(100, s.reconnectDelay || 1000);
          const delay = Math.random() * Math.min(30_000, baseDelay * Math.pow(2, attempts));
          reconnectTimerRef.current = setTimeout(reconnectFn, delay);
        } else {
          setConnectionState({
            status: 'error',
            error: attempts >= s.maxReconnectAttempts ? 'Max reconnect attempts reached' : 'Connection closed',
            reconnectAttempts: attempts,
          });
        }
      },

      onError: () => {
        // onclose fires right after — let it handle state
      },
    }),
    []
  );

  // --- Connect / reconnect ---
  useEffect(() => {
    closedIntentionallyRef.current = false;
    setConnectionState({ status: 'connecting', reconnectAttempts: 0 });

    function connectWs() {
      const wsUrl = buildWsUrl(serverRef.current);
      const handler = new TerminalWsHandler();
      handlerRef.current = handler;

      handler.callbacks = makeCallbacks(handler, () => connectWs());
      handler.connect(wsUrl, serverRef.current.authToken || undefined);
    }

    connectWs();

    return () => {
      closedIntentionallyRef.current = true;
      if (reconnectTimerRef.current) {
        clearTimeout(reconnectTimerRef.current);
      }
      if (stableTimerRef.current) {
        clearTimeout(stableTimerRef.current);
      }
      const handler = handlerRef.current;
      if (handler) {
        handler.callbacks = {};
        handler.close();
      }
      handlerRef.current = null;
      const pending = pendingCreatesRef.current;
      pendingCreatesRef.current = [];
      pending.forEach(({ reject }) => reject(new Error('WebSocket disconnected')));
      paneCallbacksRef.current.clear();
    };
  }, [server.id, server.apiUrl, server.authToken, makeCallbacks]);

  // --- Reconnection recovery: re-subscribe to all registered panes ---
  useEffect(() => {
    if (connectionState.status !== 'connected') return;
    const handler = handlerRef.current;
    if (!handler || handler.readyState !== WebSocket.OPEN) return;

    // Cross-check subscribed panes against server pane list (destroyed pane cleanup)
    const validPaneIds = new Set(paneList.map((p) => p.id));
    const subscribedPaneIds = Array.from(paneCallbacksRef.current.keys()).filter((id) => {
      if (!validPaneIds.has(id)) {
        // Pane was destroyed while disconnected - clean up callback
        paneCallbacksRef.current.get(id)?.onDestroyed();
        paneCallbacksRef.current.delete(id);
        return false;
      }
      return true;
    });

    if (subscribedPaneIds.length > 0) {
      handler.sendControl({ type: 'subscribe', paneIds: subscribedPaneIds });
    }
  }, [connectionState.status, paneList]);

  // --- Exposed API (stable references via useCallback) ---
  const subscribe = useCallback((paneId: number, callbacks: PaneCallbacks) => {
    paneCallbacksRef.current.set(paneId, callbacks);
    const handler = handlerRef.current;
    if (handler && handler.readyState === WebSocket.OPEN) {
      handler.sendControl({ type: 'subscribe', paneIds: [paneId] });
    }
  }, []);

  const unsubscribe = useCallback((paneId: number) => {
    paneCallbacksRef.current.delete(paneId);
    const handler = handlerRef.current;
    if (handler && handler.readyState === WebSocket.OPEN) {
      handler.sendControl({ type: 'unsubscribe', paneIds: [paneId] });
    }
  }, []);

  const sendInput = useCallback((paneId: number, data: string) => {
    const handler = handlerRef.current;
    if (handler && handler.readyState === WebSocket.OPEN) {
      handler.sendPtyData(paneId, data);
    }
  }, []);

  const sendResize = useCallback((paneId: number, cols: number, rows: number) => {
    const handler = handlerRef.current;
    if (handler && handler.readyState === WebSocket.OPEN) {
      handler.sendResize(paneId, cols, rows);
    }
  }, []);

  const createPane = useCallback((req: CreatePaneRequest): Promise<PaneInfoDto> => {
    return new Promise((resolve, reject) => {
      const handler = handlerRef.current;
      if (!handler || handler.readyState !== WebSocket.OPEN) {
        reject(new Error('WebSocket not connected'));
        return;
      }
      pendingCreatesRef.current.push({ resolve, reject });
      handler.sendControl({ type: 'create', ...req });
    });
  }, []);

  const switchPane = useCallback((fromPaneId: number | undefined, toPaneId: number, callbacks: PaneCallbacks) => {
    // Self-guard: no-op if switching to the same pane (L2 fix)
    if (fromPaneId != null && fromPaneId === toPaneId) return;

    // 1. Immediately update local callback map (synchronous — prevents stale snapshot delivery)
    if (fromPaneId != null) {
      paneCallbacksRef.current.delete(fromPaneId);
    }
    // Wrap onBufferSnapshot to end timing (once only — avoids console warnings on lag recovery)
    let wrappedCallbacks = callbacks;
    if (__DEV__) {
      console.time('pane-switch');
      let timed = true;
      wrappedCallbacks = {
        ...callbacks,
        onBufferSnapshot: (data: Uint8Array) => {
          if (timed) { console.timeEnd('pane-switch'); timed = false; }
          callbacks.onBufferSnapshot(data);
        },
      };
    }
    paneCallbacksRef.current.set(toPaneId, wrappedCallbacks);

    // 2. Send WS messages
    const handler = handlerRef.current;
    if (handler && handler.readyState === WebSocket.OPEN) {
      if (fromPaneId != null) {
        handler.sendControl({ type: 'unsubscribe', paneIds: [fromPaneId] });
      }
      handler.sendControl({ type: 'subscribe', paneIds: [toPaneId] });
    }
    // Buffer snapshot arrives asynchronously via onBufferSnapshot callback.
    // If toPaneId doesn't exist, server sends paneDestroyed → onDestroyed callback fires.
  }, []);

  const destroyPane = useCallback((paneId: number) => {
    const handler = handlerRef.current;
    if (handler && handler.readyState === WebSocket.OPEN) {
      handler.sendControl({ type: 'destroy', paneId });
    }
    paneCallbacksRef.current.delete(paneId);
  }, []);

  const api: TerminalWsApi = useMemo(() => ({
    subscribe,
    unsubscribe,
    switchPane,
    sendInput,
    sendResize,
    createPane,
    destroyPane,
    connectionState,
    paneList,
    paneListReady,
  }), [subscribe, unsubscribe, switchPane, sendInput, sendResize, createPane, destroyPane, connectionState, paneList, paneListReady]);

  return createElement(TerminalWsContext.Provider, { value: api }, children);
}
