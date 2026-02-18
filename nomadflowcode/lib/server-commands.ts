import type { Server, ApiResponse, SwitchFeatureResult, WorktreeStatusResponse, FileDiffResponse, FileContentResponse } from '@shared';
import type { ListSessionsResponse } from './types/session';
import type { ServerCommand, SwitchFeatureParams } from './types';

/**
 * Get the API base URL from server config
 */
function getApiBaseUrl(server: Server): string {
  // Normalize trailing slash before checking to avoid double-append (e.g. /api/ → /api/api)
  let baseUrl = (server.apiUrl || 'http://localhost:8080').replace(/\/+$/, '');

  // Ensure /api prefix is present
  if (!baseUrl.endsWith('/api')) {
    baseUrl = baseUrl + '/api';
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

    if (!response.ok) {
      const errorData = await response.json().catch(() => ({}));
      clearTimeout(timeoutId);
      return {
        success: false,
        error: errorData.detail || `HTTP ${response.status}: ${response.statusText}`,
      };
    }

    const data = await response.json();
    clearTimeout(timeoutId);
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
  const body: Record<string, string> = {
    repoPath: params.repoPath,
    featureName: params.featureName,
  };
  if (params.linkedSession) body.linkedSession = params.linkedSession;
  return executeServerCommand(server, {
    action: 'switch-feature',
    params: body,
  });
}

/**
 * Fetch worktree status (git diff stats) for a given worktree path.
 */
export async function fetchWorktreeStatus(
  server: Server,
  worktreePath: string
): Promise<ApiResponse<WorktreeStatusResponse>> {
  return executeServerCommand(server, {
    action: 'worktree-status',
    params: { worktreePath },
  });
}

/**
 * Fetch file diff (hunks) for a specific file in a worktree.
 */
export async function fetchFileDiff(
  server: Server,
  worktreePath: string,
  filePath: string
): Promise<ApiResponse<FileDiffResponse>> {
  return executeServerCommand(server, {
    action: 'file-diff',
    params: { worktreePath, filePath },
  });
}

/**
 * Fetch file content for a specific file in a worktree.
 */
export async function fetchFileContent(
  server: Server,
  worktreePath: string,
  filePath: string
): Promise<ApiResponse<FileContentResponse>> {
  return executeServerCommand(server, {
    action: 'file-content',
    params: { worktreePath, filePath },
  });
}

/**
 * Fetch all sessions with agent state from a server.
 */
export async function fetchSessions(
  server: Server
): Promise<ApiResponse<ListSessionsResponse>> {
  return executeServerCommand(server, {
    action: 'list-sessions',
  });
}

/**
 * Close an agent session (kills the tmux window).
 */
export async function closeSession(
  server: Server,
  sessionId: string
): Promise<ApiResponse<{ closed: boolean }>> {
  return executeServerCommand(server, {
    action: 'close-session',
    params: { sessionId },
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
