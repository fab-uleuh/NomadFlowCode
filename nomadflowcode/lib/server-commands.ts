import type { Server, ApiResponse, SwitchFeatureResult } from '@shared';
import type { ServerCommand, SwitchFeatureParams } from './types';

/**
 * Get the API base URL from server config
 */
function getApiBaseUrl(server: Server): string {
  let baseUrl = server.apiUrl || 'http://localhost:8080';

  // Ensure /api prefix is present
  if (!baseUrl.endsWith('/api')) {
    baseUrl = baseUrl.replace(/\/$/, '') + '/api';
  }

  return baseUrl;
}

/**
 * Execute a command on the server via REST API
 */
export async function executeServerCommand(
  server: Server,
  command: ServerCommand
): Promise<ApiResponse<any>> {
  const timeout = command.action === 'clone-repo' ? 60000 : 10000;

  const baseUrl = getApiBaseUrl(server);

  const controller = new AbortController();
  const timeoutId = setTimeout(() => controller.abort(), timeout);

  try {
    const response = await fetch(`${baseUrl}/${command.action}`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        ...(server.authToken ? { Authorization: `Bearer ${server.authToken}` } : {}),
      },
      body: JSON.stringify(command.params || {}),
      signal: controller.signal,
    });

    clearTimeout(timeoutId);

    if (!response.ok) {
      const errorData = await response.json().catch(() => ({}));
      return {
        success: false,
        error: errorData.detail || `HTTP ${response.status}: ${response.statusText}`,
      };
    }

    const data = await response.json();
    return { success: true, data };
  } catch (error) {
    clearTimeout(timeoutId);
    const message = error instanceof Error ? error.message : 'Unknown error';
    console.error('API request failed:', message);
    return {
      success: false,
      error: message === 'The operation was aborted' ? 'Request timeout' : message,
    };
  }
}

/**
 * Switch to a feature worktree before opening the terminal.
 * This ensures the tmux window is selected and cd'd into the correct directory.
 * If a process (like claude) is already running, no commands are sent to avoid interference.
 */
export async function switchToFeature(
  server: Server,
  params: SwitchFeatureParams
): Promise<ApiResponse<SwitchFeatureResult>> {
  return executeServerCommand(server, {
    action: 'switch-feature',
    params: {
      repoPath: params.repoPath,
      featureName: params.featureName,
    },
  });
}

/**
 * Build the initialization commands to send to the terminal
 */
export function buildInitCommands(
  repoPath: string,
  featureName: string,
  worktreePath: string,
  tmuxSessionPrefix: string,
  aiAgentCommand?: string
): string[] {
  const sessionName = tmuxSessionPrefix;
  const windowName = featureName;

  const commands = [
    `tmux has-session -t "${sessionName}" 2>/dev/null || tmux new-session -d -s "${sessionName}"`,
    `tmux select-window -t "${sessionName}:${windowName}" 2>/dev/null || tmux new-window -t "${sessionName}" -n "${windowName}"`,
    `cd "${worktreePath}" 2>/dev/null || cd ~`,
    'git status --short 2>/dev/null || echo "Not a git repository"',
    'clear',
    `echo "🚀 NomadFlow Terminal"`,
    `echo "📂 Feature: ${featureName}"`,
    `echo "🌿 Path: ${worktreePath}"`,
    `echo ""`,
  ];

  if (aiAgentCommand) {
    commands.push(`echo "🤖 Launching AI assistant..."`);
    commands.push(`echo ""`);
    commands.push(aiAgentCommand);
  }

  return commands;
}

/**
 * Generate tmux commands for common operations
 */
export const tmuxCommands = {
  listWindows: 'tmux list-windows',
  newWindow: (name: string) => `tmux new-window -n "${name}"`,
  selectWindow: (name: string) => `tmux select-window -t "${name}"`,
  killWindow: (name: string) => `tmux kill-window -t "${name}"`,
  splitHorizontal: 'tmux split-window -h',
  splitVertical: 'tmux split-window -v',
  nextPane: 'tmux select-pane -t :.+',
  prevPane: 'tmux select-pane -t :.-',
  zoomPane: 'tmux resize-pane -Z',
  detach: 'tmux detach',
  scrollMode: 'tmux copy-mode',
};
