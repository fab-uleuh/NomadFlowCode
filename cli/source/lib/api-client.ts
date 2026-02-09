import type {
  Server,
  ApiResponse,
  RepoListResponse,
  FeatureListResponse,
  SwitchFeatureResult,
  CreateFeatureResult,
} from '../types.js';

function getApiBaseUrl(server: Server): string {
  let baseUrl = server.apiUrl;

  if (!baseUrl) {
    const ttydUrl = server.ttydUrl || 'http://localhost:7681';
    try {
      const url = new URL(ttydUrl);
      url.port = url.port === '7681' ? '8080' : url.port;
      baseUrl = url.toString().replace(/\/$/, '');
    } catch {
      baseUrl = ttydUrl.replace(':7681', ':8080');
    }
  }

  if (!baseUrl.endsWith('/api')) {
    baseUrl = baseUrl.replace(/\/$/, '') + '/api';
  }

  return baseUrl;
}

async function apiRequest<T>(
  server: Server,
  action: string,
  params: Record<string, string | boolean | undefined> = {},
  timeout = 10000,
): Promise<ApiResponse<T>> {
  const baseUrl = getApiBaseUrl(server);
  const controller = new AbortController();
  const timeoutId = setTimeout(() => controller.abort(), timeout);

  try {
    const response = await fetch(`${baseUrl}/${action}`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        ...(server.authToken ? { Authorization: `Bearer ${server.authToken}` } : {}),
      },
      body: JSON.stringify(params),
      signal: controller.signal,
    });

    clearTimeout(timeoutId);

    if (!response.ok) {
      const errorData = await response.json().catch(() => ({}));
      return {
        success: false,
        error: (errorData as any).detail || `HTTP ${response.status}: ${response.statusText}`,
      };
    }

    const data = (await response.json()) as T;
    return { success: true, data };
  } catch (error) {
    clearTimeout(timeoutId);
    const message = error instanceof Error ? error.message : 'Unknown error';
    return {
      success: false,
      error: message.includes('abort') ? 'Request timeout' : message,
    };
  }
}

export async function checkHealth(server: Server): Promise<boolean> {
  const baseUrl = getApiBaseUrl(server).replace('/api', '');
  try {
    const controller = new AbortController();
    const timeoutId = setTimeout(() => controller.abort(), 3000);
    const response = await fetch(`${baseUrl}/health`, {
      signal: controller.signal,
      headers: server.authToken ? { Authorization: `Bearer ${server.authToken}` } : {},
    });
    clearTimeout(timeoutId);
    return response.ok;
  } catch {
    return false;
  }
}

export async function listRepos(server: Server): Promise<ApiResponse<RepoListResponse>> {
  return apiRequest<RepoListResponse>(server, 'list-repos');
}

export async function listFeatures(
  server: Server,
  repoPath: string,
): Promise<ApiResponse<FeatureListResponse>> {
  return apiRequest<FeatureListResponse>(server, 'list-features', { repoPath });
}

export async function switchFeature(
  server: Server,
  repoPath: string,
  featureName: string,
): Promise<ApiResponse<SwitchFeatureResult>> {
  return apiRequest<SwitchFeatureResult>(server, 'switch-feature', { repoPath, featureName });
}

export async function createFeature(
  server: Server,
  repoPath: string,
  featureName: string,
  baseBranch?: string,
): Promise<ApiResponse<CreateFeatureResult>> {
  return apiRequest<CreateFeatureResult>(server, 'create-feature', {
    repoPath,
    featureName,
    baseBranch,
  });
}
