"""Terminal WebSocket proxy router.

Proxies only the WebSocket connection to ttyd through the API server.
The ttyd HTML page is loaded directly by the mobile app (with basicAuthCredential),
but WKWebView does not propagate HTTP Basic Auth to WebSocket upgrade requests,
so we proxy the WebSocket here with server-side Basic Auth.

Flow:
  1. Mobile app loads http://ttyd:7681/ directly (basicAuthCredential handles auth)
  2. Injected JS rewrites the WebSocket URL to ws://API:8080/terminal/ws?token=...
  3. FastAPI verifies token, then proxies WebSocket to ttyd with Basic Auth
"""

import asyncio
import base64

from fastapi import APIRouter, WebSocket, WebSocketDisconnect
from websockets.asyncio.client import connect as ws_connect
from websockets.exceptions import ConnectionClosed

from ..config import get_settings

router = APIRouter(tags=["terminal"])


def _ttyd_auth_headers() -> dict[str, str]:
    """Build HTTP Basic Auth headers for the server-side connection to ttyd."""
    settings = get_settings()
    if not settings.auth.secret:
        return {}
    creds = base64.b64encode(f"nomadflow:{settings.auth.secret}".encode()).decode()
    return {"Authorization": f"Basic {creds}"}


# --- WebSocket proxy route (token auth via query parameter) ---


@router.websocket("/terminal/ws")
async def proxy_ttyd_ws(websocket: WebSocket):
    """Proxy WebSocket to ttyd, handling auth on the server side."""
    settings = get_settings()

    # Verify auth via query parameter (WebView JS injects this)
    if settings.auth.secret:
        token = websocket.query_params.get("token", "")
        if token != settings.auth.secret:
            await websocket.close(code=1008, reason="Authentication required")
            return

    # Accept with subprotocol if the client requested one (ttyd uses "tty")
    requested = websocket.headers.get("sec-websocket-protocol", "")
    subprotocols = [s.strip() for s in requested.split(",") if s.strip()]
    if subprotocols:
        await websocket.accept(subprotocol=subprotocols[0])
    else:
        await websocket.accept()

    ws_url = f"ws://127.0.0.1:{settings.ttyd.port}/ws"
    headers = _ttyd_auth_headers()

    try:
        async with ws_connect(
            ws_url,
            additional_headers=headers,
            subprotocols=subprotocols if subprotocols else None,
        ) as ttyd_ws:
            print(f"[Terminal Proxy] Connected to ttyd, subprotocol={ttyd_ws.subprotocol}")

            async def client_to_ttyd():
                """Relay messages from the mobile app to ttyd."""
                try:
                    while True:
                        msg = await websocket.receive()
                        msg_type = msg.get("type", "")
                        if msg_type == "websocket.disconnect":
                            break
                        if "bytes" in msg and msg["bytes"]:
                            await ttyd_ws.send(msg["bytes"])
                        elif "text" in msg and msg["text"]:
                            await ttyd_ws.send(msg["text"])
                except (WebSocketDisconnect, Exception):
                    pass

            async def ttyd_to_client():
                """Relay messages from ttyd to the mobile app."""
                count = 0
                try:
                    async for msg in ttyd_ws:
                        count += 1
                        if count <= 3:
                            print(f"[Terminal Proxy] ttyd->client msg #{count}: "
                                  f"type={'bytes' if isinstance(msg, bytes) else 'text'}, "
                                  f"len={len(msg)}")
                        if isinstance(msg, bytes):
                            await websocket.send_bytes(msg)
                        else:
                            await websocket.send_text(msg)
                except (ConnectionClosed, Exception) as e:
                    print(f"[Terminal Proxy] ttyd->client ended after {count} msgs: {e}")

            done, pending = await asyncio.wait(
                [
                    asyncio.create_task(client_to_ttyd()),
                    asyncio.create_task(ttyd_to_client()),
                ],
                return_when=asyncio.FIRST_COMPLETED,
            )
            for task in pending:
                task.cancel()
    except Exception as e:
        print(f"[Terminal Proxy] WebSocket error: {e}")
    finally:
        try:
            await websocket.close()
        except Exception:
            pass
