"""Async subprocess helper for running shell commands."""

import asyncio
import shlex
from dataclasses import dataclass
from typing import Sequence


@dataclass
class CommandResult:
    """Result of a shell command execution."""

    stdout: str
    stderr: str
    return_code: int

    @property
    def success(self) -> bool:
        """Check if the command succeeded."""
        return self.return_code == 0

    @property
    def output(self) -> str:
        """Get the combined output (stdout preferred, stderr as fallback)."""
        return self.stdout.strip() if self.stdout.strip() else self.stderr.strip()


async def run_command(
    command: str | Sequence[str],
    cwd: str | None = None,
    timeout: float = 30.0,
    env: dict[str, str] | None = None,
) -> CommandResult:
    """
    Run a shell command asynchronously.

    Args:
        command: Command to run (string or list of arguments)
        cwd: Working directory for the command
        timeout: Timeout in seconds
        env: Additional environment variables

    Returns:
        CommandResult with stdout, stderr, and return code
    """
    if isinstance(command, str):
        # Use shell=True for string commands
        process = await asyncio.create_subprocess_shell(
            command,
            stdout=asyncio.subprocess.PIPE,
            stderr=asyncio.subprocess.PIPE,
            cwd=cwd,
            env=env,
        )
    else:
        # Use shell=False for list commands
        process = await asyncio.create_subprocess_exec(
            *command,
            stdout=asyncio.subprocess.PIPE,
            stderr=asyncio.subprocess.PIPE,
            cwd=cwd,
            env=env,
        )

    try:
        stdout, stderr = await asyncio.wait_for(
            process.communicate(),
            timeout=timeout,
        )
    except asyncio.TimeoutError:
        process.kill()
        await process.wait()
        return CommandResult(
            stdout="",
            stderr=f"Command timed out after {timeout} seconds",
            return_code=-1,
        )

    return CommandResult(
        stdout=stdout.decode("utf-8", errors="replace"),
        stderr=stderr.decode("utf-8", errors="replace"),
        return_code=process.returncode or 0,
    )


async def command_exists(command_name: str) -> bool:
    """Check if a command exists in PATH."""
    result = await run_command(f"which {shlex.quote(command_name)}")
    return result.success
