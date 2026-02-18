import { executeServerCommand } from '@/lib/server-commands';
import type { Server } from '@shared';
import type { ServerCommand } from '@/lib/types';

const mockFetch = global.fetch as jest.Mock;

function makeServer(overrides: Partial<Server> = {}): Server {
  return {
    id: 'test-server',
    name: 'Test',
    apiUrl: 'http://localhost:8080',
    ...overrides,
  };
}

describe('executeServerCommand', () => {
  beforeEach(() => {
    mockFetch.mockReset();
  });

  it('returns { success: true, data } on successful API call', async () => {
    const responseData = { repos: [{ name: 'my-repo', path: '/tmp/repo', branch: 'main' }] };
    mockFetch.mockResolvedValueOnce({
      ok: true,
      json: async () => responseData,
    });

    const result = await executeServerCommand(makeServer(), { action: 'list-repos' });

    expect(result.success).toBe(true);
    expect(result.data).toEqual(responseData);
    expect(mockFetch).toHaveBeenCalledWith(
      'http://localhost:8080/api/list-repos',
      expect.objectContaining({ method: 'POST' })
    );
  });

  it('returns { success: false, error } with status text on HTTP error', async () => {
    mockFetch.mockResolvedValueOnce({
      ok: false,
      status: 403,
      statusText: 'Forbidden',
      json: async () => ({}),
    });

    const result = await executeServerCommand(makeServer(), { action: 'list-repos' });

    expect(result.success).toBe(false);
    expect(result.error).toContain('403');
    expect(result.error).toContain('Forbidden');
  });

  it('returns { success: false, error: "Request timeout" } on abort', async () => {
    jest.spyOn(console, 'error').mockImplementation(() => {});
    mockFetch.mockRejectedValueOnce(new Error('The operation was aborted'));

    const result = await executeServerCommand(makeServer(), { action: 'list-repos' });

    expect(result.success).toBe(false);
    expect(result.error).toBe('Request timeout');
  });

  it('clone-repo uses 60s timeout (not aborted at 10s)', async () => {
    jest.useFakeTimers();
    jest.spyOn(console, 'error').mockImplementation(() => {});

    let abortSignal: AbortSignal | undefined;
    mockFetch.mockImplementationOnce((_url: string, opts: RequestInit) => {
      abortSignal = opts.signal as AbortSignal;
      return new Promise((_resolve, reject) => {
        opts.signal?.addEventListener('abort', () => reject(new Error('The operation was aborted')));
      });
    });

    const resultPromise = executeServerCommand(makeServer(), {
      action: 'clone-repo',
      params: { url: 'https://github.com/test/repo' },
    });

    // At 10s, clone-repo should NOT be aborted
    jest.advanceTimersByTime(10000);
    expect(abortSignal?.aborted).toBe(false);

    // At 60s, it should be aborted
    jest.advanceTimersByTime(50000);
    expect(abortSignal?.aborted).toBe(true);

    const result = await resultPromise;
    expect(result.success).toBe(false);
    expect(result.error).toBe('Request timeout');

    jest.useRealTimers();
  });

  it('list-repos uses 10s timeout', async () => {
    jest.useFakeTimers();
    jest.spyOn(console, 'error').mockImplementation(() => {});

    let abortSignal: AbortSignal | undefined;
    mockFetch.mockImplementationOnce((_url: string, opts: RequestInit) => {
      abortSignal = opts.signal as AbortSignal;
      return new Promise((_resolve, reject) => {
        opts.signal?.addEventListener('abort', () => reject(new Error('The operation was aborted')));
      });
    });

    const resultPromise = executeServerCommand(makeServer(), { action: 'list-repos' });

    // At 5s, should NOT be aborted
    jest.advanceTimersByTime(5000);
    expect(abortSignal?.aborted).toBe(false);

    // At 10s, should be aborted
    jest.advanceTimersByTime(5000);
    expect(abortSignal?.aborted).toBe(true);

    const result = await resultPromise;
    expect(result.success).toBe(false);
    expect(result.error).toBe('Request timeout');

    jest.useRealTimers();
  });

  it('includes auth header when server.authToken is set', async () => {
    mockFetch.mockResolvedValueOnce({
      ok: true,
      json: async () => ({ repos: [] }),
    });

    await executeServerCommand(makeServer({ authToken: 'my-secret' }), {
      action: 'list-repos',
    });

    expect(mockFetch).toHaveBeenCalledWith(
      expect.any(String),
      expect.objectContaining({
        headers: expect.objectContaining({
          Authorization: 'Bearer my-secret',
        }),
      })
    );
  });

  it('omits auth header when server.authToken is not set', async () => {
    mockFetch.mockResolvedValueOnce({
      ok: true,
      json: async () => ({ repos: [] }),
    });

    await executeServerCommand(makeServer({ authToken: undefined }), {
      action: 'list-repos',
    });

    const callHeaders = mockFetch.mock.calls[0][1].headers;
    expect(callHeaders).not.toHaveProperty('Authorization');
  });
});
