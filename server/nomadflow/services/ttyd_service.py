"""Ttyd subprocess management service."""

import asyncio
from typing import Optional

import psutil

from ..config import Settings
from ..utils.shell import run_command, command_exists


class TtydService:
    """Service for managing the ttyd subprocess."""

    def __init__(self, settings: Settings):
        self.settings = settings
        self.port = settings.ttyd.port
        self.session_name = settings.tmux.session
        self._process: Optional[asyncio.subprocess.Process] = None

    async def start(self) -> bool:
        """Start the ttyd subprocess."""
        # Check if ttyd is available
        if not await command_exists("ttyd"):
            raise RuntimeError(
                "ttyd is not installed or not in PATH. "
                "Install with: brew install ttyd (macOS) or apt install ttyd (Linux)"
            )

        # Check if port is already in use
        if await self._port_in_use():
            print(f"ttyd already running on port {self.port}")
            return True

        # Start ttyd attached to tmux session
        cmd = ["ttyd", "-p", str(self.port), "-W"]

        # Add basic auth if secret is configured
        if self.settings.auth.secret:
            cmd.extend(["-c", f"nomadflow:{self.settings.auth.secret}"])

        cmd.extend(["tmux", "attach-session", "-t", self.session_name])

        self._process = await asyncio.create_subprocess_exec(
            *cmd,
            stdout=asyncio.subprocess.PIPE,
            stderr=asyncio.subprocess.PIPE,
        )

        # Give it a moment to start
        await asyncio.sleep(0.5)

        # Check if it started successfully
        if self._process.returncode is not None:
            stderr = await self._process.stderr.read() if self._process.stderr else b""
            raise RuntimeError(f"ttyd failed to start: {stderr.decode()}")

        print(f"ttyd started on port {self.port}")
        return True

    async def stop(self) -> bool:
        """Stop the ttyd subprocess."""
        if self._process:
            try:
                self._process.terminate()
                await asyncio.wait_for(self._process.wait(), timeout=5.0)
            except asyncio.TimeoutError:
                self._process.kill()
                await self._process.wait()
            self._process = None
            print("ttyd stopped")
            return True

        # Try to find and kill any existing ttyd process on our port
        await self._kill_existing()
        return True

    async def _port_in_use(self) -> bool:
        """Check if the ttyd port is already in use."""
        import socket
        sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        try:
            sock.bind(('127.0.0.1', self.port))
            sock.close()
            return False  # Port is free
        except OSError:
            return True  # Port is in use

    async def _kill_existing(self) -> None:
        """Kill any existing ttyd process on our port."""
        for proc in psutil.process_iter(["pid", "name", "cmdline"]):
            try:
                if proc.info["name"] == "ttyd":
                    cmdline = proc.info["cmdline"] or []
                    if str(self.port) in cmdline:
                        proc.terminate()
                        proc.wait(timeout=5)
            except (psutil.NoSuchProcess, psutil.AccessDenied, psutil.TimeoutExpired):
                pass

    @property
    def is_running(self) -> bool:
        """Check if ttyd is currently running."""
        if self._process:
            return self._process.returncode is None
        return False

    @property
    def websocket_url(self) -> str:
        """Get the WebSocket URL for the ttyd instance."""
        return f"ws://localhost:{self.port}/ws"
