import { execSync } from 'node:child_process';
import type { TmuxWindow } from '../types.js';

function exec(cmd: string): string | null {
  try {
    return execSync(cmd, { encoding: 'utf-8', timeout: 5000 }).trim();
  } catch {
    return null;
  }
}

export function isTmuxInstalled(): boolean {
  return exec('which tmux') !== null;
}

export function sessionExists(sessionName: string): boolean {
  return exec(`tmux has-session -t "${sessionName}" 2>/dev/null`) !== null;
}

export function listWindows(sessionName: string): TmuxWindow[] {
  const output = exec(
    `tmux list-windows -t "${sessionName}" -F "#{window_index}:#{window_name}:#{window_active}"`,
  );

  if (!output) return [];

  return output
    .split('\n')
    .filter(Boolean)
    .map(line => {
      const [index, name, active] = line.split(':');
      return {
        index: parseInt(index!, 10),
        name: name!,
        active: active === '1',
      };
    });
}

export function getPaneCommand(sessionName: string, windowName: string): string | null {
  return exec(
    `tmux list-panes -t "${sessionName}:${windowName}" -F "#{pane_current_command}"`,
  );
}

export function isShellIdle(sessionName: string, windowName: string): boolean {
  const command = getPaneCommand(sessionName, windowName);
  if (!command) return true;

  const idleShells = new Set(['bash', 'zsh', 'sh', 'fish', 'dash', 'ksh', 'tcsh', 'csh']);
  return idleShells.has(command.toLowerCase().split('\n')[0]!);
}

export function attachSession(sessionName: string): void {
  // Replace the current process with tmux attach
  // This must be called with execSync in a way that takes over the terminal
  try {
    execSync(`tmux attach-session -t "${sessionName}"`, {
      stdio: 'inherit',
    });
  } catch {
    // User detached from tmux, which is normal
  }
}
