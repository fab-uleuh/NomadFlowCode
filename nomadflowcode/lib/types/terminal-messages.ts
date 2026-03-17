import type { AgentStateKind } from './session';
export type { AgentStateKind };

/** Pane info received from the bundled terminal HTML via postMessage. */
export interface Pane {
  id: number;
  label: string;
  repo: string;
  worktree: string;
  agentType: string;
  agentNumber: number;
  cols: number;
  rows: number;
  cwd: string;
  agentState: AgentStateKind;
}

/** Payload for creating a new pane via the server API. */
export interface CreatePaneRequest {
  repo: string;
  worktree: string;
  agentType: string;
  cwd: string;
}

/**
 * Messages sent from React Native to the terminal WebView.
 * Discriminated union on the `type` field.
 */
export type NativeToWebMessage =
  | { type: 'connect'; wsUrl: string; token: string; paneLabel?: string; repo?: string; worktree?: string; agentType?: string; cwd?: string; fontSize?: number }
  | { type: 'switchPane'; paneId: number }
  | { type: 'sendInput'; data: string }
  | { type: 'resize' }
  | { type: 'setFontSize'; fontSize: number }
  | { type: 'createPane'; request: CreatePaneRequest }
  | { type: 'destroyPane'; paneId: number }
  | { type: 'disconnect' }
  | { type: 'reconnect' }
  | { type: 'blur' };

/**
 * Messages sent from the terminal WebView to React Native.
 * Discriminated union on the `type` field.
 *
 * Note: `scroll_state` is intentionally excluded — it's a debug artifact
 * handled as a no-op in the default case.
 */
export type WebToNativeMessage =
  | { type: 'connected' }
  | { type: 'disconnected' }
  | { type: 'paneSwitched'; paneId: number; label: string }
  | { type: 'paneList'; panes: Pane[] }
  | { type: 'paneDestroyed'; paneId: number }
  | { type: 'paneStateUpdated'; paneId: number; agentState: AgentStateKind }
  | { type: 'resized'; cols: number; rows: number }
  | { type: 'font_size'; fontSize: number }
  | { type: 'reconnecting'; attempt: number; maxAttempts: number }
  | { type: 'error'; message: string };
