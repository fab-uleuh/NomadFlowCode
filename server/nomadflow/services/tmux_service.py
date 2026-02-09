"""Tmux session and window management service."""

from ..config import Settings
from ..utils.shell import run_command, command_exists


class TmuxService:
    """Service for managing tmux sessions and windows."""

    def __init__(self, settings: Settings):
        self.settings = settings
        self.session_name = settings.tmux.session

    async def ensure_session(self) -> bool:
        """Ensure the tmux session exists, create if not."""
        # Check if tmux is available
        if not await command_exists("tmux"):
            raise RuntimeError("tmux is not installed or not in PATH")

        # Check if session exists
        result = await run_command(f'tmux has-session -t "{self.session_name}" 2>/dev/null')

        if not result.success:
            # Create new session
            result = await run_command(
                f'tmux new-session -d -s "{self.session_name}"'
            )
            if not result.success:
                raise RuntimeError(f"Failed to create tmux session: {result.stderr}")

        return True

    async def list_windows(self) -> list[dict[str, str]]:
        """List all windows in the session."""
        result = await run_command(
            f'tmux list-windows -t "{self.session_name}" -F "#{{window_index}}:#{{window_name}}"'
        )

        windows = []
        if result.success:
            for line in result.stdout.strip().split("\n"):
                if ":" in line:
                    index, name = line.split(":", 1)
                    windows.append({"index": index, "name": name})

        return windows

    async def create_window(self, name: str, working_dir: str | None = None) -> str:
        """Create a new window in the session."""
        cmd = f'tmux new-window -t "{self.session_name}" -n "{name}"'
        if working_dir:
            cmd += f' -c "{working_dir}"'

        result = await run_command(cmd)
        if not result.success:
            raise RuntimeError(f"Failed to create tmux window: {result.stderr}")

        return name

    async def select_window(self, name: str) -> bool:
        """Select/focus a window by name."""
        result = await run_command(
            f'tmux select-window -t "{self.session_name}:{name}"'
        )
        return result.success

    async def kill_window(self, name: str) -> bool:
        """Kill a window by name."""
        result = await run_command(
            f'tmux kill-window -t "{self.session_name}:{name}"'
        )
        return result.success

    async def send_keys(self, window: str, keys: str, enter: bool = True) -> bool:
        """Send keys to a window."""
        cmd = f'tmux send-keys -t "{self.session_name}:{window}" "{keys}"'
        if enter:
            cmd += " Enter"

        result = await run_command(cmd)
        return result.success

    async def window_exists(self, name: str) -> bool:
        """Check if a window exists."""
        windows = await self.list_windows()
        return any(w["name"] == name for w in windows)

    async def get_pane_command(self, window: str) -> str | None:
        """Get the current command running in the window's active pane.

        Returns the name of the process (e.g., 'bash', 'zsh', 'claude', 'python').
        Returns None if the window doesn't exist.
        """
        result = await run_command(
            f'tmux list-panes -t "{self.session_name}:{window}" -F "#{{pane_current_command}}"'
        )
        if result.success and result.stdout.strip():
            # Return the first pane's command (usually the only one)
            return result.stdout.strip().split("\n")[0]
        return None

    async def is_shell_idle(self, window: str) -> bool:
        """Check if the window has an idle shell (no interactive process running).

        Returns True if the pane is running a shell (bash, zsh, sh, fish, etc.)
        Returns False if an interactive process like claude, python, vim, etc. is running.
        """
        command = await self.get_pane_command(window)
        if command is None:
            return True  # Window doesn't exist yet, will be created fresh

        # Common shell names that indicate an idle shell
        idle_shells = {"bash", "zsh", "sh", "fish", "dash", "ksh", "tcsh", "csh"}
        return command.lower() in idle_shells

    async def ensure_window(self, name: str, working_dir: str | None = None) -> str:
        """Ensure a window exists, create if not."""
        if not await self.window_exists(name):
            await self.create_window(name, working_dir)
            if working_dir:
                # Also cd into the directory
                await self.send_keys(name, f'cd "{working_dir}"')

        return name

    async def switch_to_window(
        self, name: str, working_dir: str | None = None
    ) -> tuple[bool, bool]:
        """Switch to a window and optionally cd into a directory.

        This method:
        1. Checks if a process is already running in the window
        2. Ensures the window exists (creates if needed)
        3. Selects/focuses the window
        4. Only if shell is idle: sends cd command and clears the terminal

        Args:
            name: The window name
            working_dir: Optional directory to cd into

        Returns:
            Tuple of (switched_successfully, has_running_process)
            - switched_successfully: True if window was selected
            - has_running_process: True if an interactive process is running
        """
        # Check if a process is already running before ensuring window exists
        window_existed = await self.window_exists(name)
        has_running_process = False

        if window_existed:
            # Check if shell is idle (no process running)
            is_idle = await self.is_shell_idle(name)
            has_running_process = not is_idle

        # Ensure window exists (creates if needed with working_dir)
        await self.ensure_window(name, working_dir)

        # Select the window
        selected = await self.select_window(name)
        if not selected:
            return False, has_running_process

        # Only CD and clear if:
        # - Window was just created (window_existed is False), OR
        # - Shell is idle (no process running)
        if working_dir and not has_running_process:
            await self.send_keys(name, f'cd "{working_dir}"')
            await self.send_keys(name, 'clear')

        return True, has_running_process
