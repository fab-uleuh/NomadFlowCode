/**
 * terminal-ws.ts — TypeScript client library for the NomadFlow multiplexed binary WS protocol.
 *
 * Frame format: [Type 1B][PaneID 2B big-endian][Payload variable]
 * Matches protocol.rs exactly.
 */

// Frame type constants (matching protocol.rs)
export const PTY_DATA = 0x01;
export const RESIZE = 0x02;
export const CONTROL = 0x03;
export const BUFFER_SNAPSHOT = 0x04;
export const PING = 0x05;
export const CONTROL_PANE_ID = 0x0000;

// --- TypeScript types matching Rust protocol.rs ---

export interface PaneInfoDto {
  id: number;
  label: string;
  repo: string;
  worktree: string;
  agentType: string;
  agentNumber: number;
  cols: number;
  rows: number;
  cwd: string;
}

export interface CreatePaneRequest {
  repo: string;
  worktree: string;
  agentType: string;
  cwd: string;
  cols?: number;
  rows?: number;
  shell?: string;
}

export type ControlMsg =
  | { type: 'list' }
  | ({ type: 'create' } & CreatePaneRequest)
  | { type: 'destroy'; paneId: number }
  | { type: 'subscribe'; paneIds: number[] }
  | { type: 'unsubscribe'; paneIds: number[] }
  | { type: 'paneList'; panes: PaneInfoDto[] }
  | { type: 'error'; message: string }
  | ({ type: 'paneCreated' } & PaneInfoDto)
  | { type: 'paneDestroyed'; paneId: number };

export interface WsFrame {
  type: number;
  paneId: number;
  payload: Uint8Array;
}

// --- Frame encoding / decoding ---

export function encodeFrame(type: number, paneId: number, payload: Uint8Array): ArrayBuffer {
  const frame = new Uint8Array(3 + payload.length);
  frame[0] = type;
  frame[1] = (paneId >> 8) & 0xff;
  frame[2] = paneId & 0xff;
  frame.set(payload, 3);
  return frame.buffer;
}

export function decodeFrame(data: ArrayBuffer): WsFrame {
  const bytes = new Uint8Array(data);
  if (bytes.length < 3) throw new Error('Frame too short');
  return {
    type: bytes[0],
    paneId: (bytes[1] << 8) | bytes[2],
    payload: bytes.slice(3),
  };
}

function encodeResize(cols: number, rows: number): Uint8Array {
  const payload = new Uint8Array(4);
  payload[0] = (cols >> 8) & 0xff;
  payload[1] = cols & 0xff;
  payload[2] = (rows >> 8) & 0xff;
  payload[3] = rows & 0xff;
  return payload;
}

// --- TerminalWsHandler class ---

export interface TerminalWsHandlerCallbacks {
  onPtyData?: (paneId: number, data: Uint8Array) => void;
  onBufferSnapshot?: (paneId: number, data: Uint8Array) => void;
  onControl?: (msg: ControlMsg) => void;
  onOpen?: () => void;
  onClose?: (code: number, reason: string) => void;
  onError?: (event: Event) => void;
}

export class TerminalWsHandler {
  private ws: WebSocket | null = null;
  private encoder = new TextEncoder();
  private decoder = new TextDecoder();
  public callbacks: TerminalWsHandlerCallbacks = {};

  get readyState(): number {
    return this.ws?.readyState ?? WebSocket.CLOSED;
  }

  connect(url: string, token?: string): void {
    let wsUrl = url;
    if (token) {
      wsUrl += `?token=${encodeURIComponent(token)}`;
    }
    const ws = new WebSocket(wsUrl);
    ws.binaryType = 'arraybuffer';
    this.ws = ws;

    ws.onopen = () => {
      this.callbacks.onOpen?.();
    };

    ws.onmessage = (event: MessageEvent) => {
      if (!(event.data instanceof ArrayBuffer)) return;
      try {
        const frame = decodeFrame(event.data);
        switch (frame.type) {
          case PTY_DATA:
            this.callbacks.onPtyData?.(frame.paneId, frame.payload);
            break;
          case BUFFER_SNAPSHOT:
            this.callbacks.onBufferSnapshot?.(frame.paneId, frame.payload);
            break;
          case CONTROL: {
            const msg: ControlMsg = JSON.parse(this.decoder.decode(frame.payload));
            this.callbacks.onControl?.(msg);
            break;
          }
          case PING:
            // Respond with pong (same frame back)
            ws.send(encodeFrame(PING, CONTROL_PANE_ID, new Uint8Array(0)));
            break;
        }
      } catch (err) {
        console.error('[TerminalWsHandler] Frame decode error:', err);
      }
    };

    ws.onclose = (event: CloseEvent) => {
      this.callbacks.onClose?.(event.code, event.reason);
    };

    ws.onerror = (event: Event) => {
      this.callbacks.onError?.(event);
    };
  }

  close(): void {
    this.ws?.close();
    this.ws = null;
  }

  sendPtyData(paneId: number, data: string): void {
    if (!this.ws || this.ws.readyState !== WebSocket.OPEN) return;
    const encoded = this.encoder.encode(data);
    this.ws.send(encodeFrame(PTY_DATA, paneId, encoded));
  }

  sendResize(paneId: number, cols: number, rows: number): void {
    if (!this.ws || this.ws.readyState !== WebSocket.OPEN) return;
    this.ws.send(encodeFrame(RESIZE, paneId, encodeResize(cols, rows)));
  }

  sendControl(msg: ControlMsg): void {
    if (!this.ws || this.ws.readyState !== WebSocket.OPEN) return;
    const json = this.encoder.encode(JSON.stringify(msg));
    this.ws.send(encodeFrame(CONTROL, CONTROL_PANE_ID, json));
  }
}
