import type { Server, ApiResponse, SwitchFeatureResult, WorktreeStatusResponse, FileDiffResponse, FileContentResponse, ListDirResponse } from '@shared';
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
 * This ensures the worktree exists and is ready for a PTY session.
 */
export async function switchToFeature(
  server: Server,
  params: SwitchFeatureParams
): Promise<ApiResponse<SwitchFeatureResult>> {
  const body: Record<string, string> = {
    repoPath: params.repoPath,
    featureName: params.featureName,
  };
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
 * Fetch directory listing for a worktree path.
 */
export async function fetchListDir(
  server: Server,
  worktreePath: string,
  relativePath: string
): Promise<ApiResponse<ListDirResponse>> {
  return executeServerCommand(server, {
    action: 'list-dir',
    params: { worktreePath, relativePath },
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
 * Close an agent session (kills the PTY pane).
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
 * List all active PTY panes on a server.
 */
export async function fetchPanes(
  server: Server
): Promise<ApiResponse<{ panes: import('@/lib/types/terminal-messages').Pane[] }>> {
  return executeServerCommand(server, {
    action: 'list-panes',
  });
}

/**
 * Destroy a PTY pane on the server.
 */
export async function destroyPane(
  server: Server,
  paneId: number
): Promise<ApiResponse<{ destroyed: boolean }>> {
  return executeServerCommand(server, {
    action: 'destroy-pane',
    params: { paneId },
  });
}

