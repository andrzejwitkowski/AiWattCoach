import { afterEach, describe, expect, it, vi } from 'vitest';

import { loadCurrentUser } from './auth';

const originalFetch = global.fetch;

afterEach(() => {
  global.fetch = originalFetch;
  vi.restoreAllMocks();
});

describe('loadCurrentUser', () => {
  it('includes credentials and returns an authenticated user payload', async () => {
    const fetchMock = vi.fn<(input: RequestInfo | URL, init?: RequestInit) => Promise<Response>>()
      .mockResolvedValue(
        new Response(
          JSON.stringify({
            authenticated: true,
            user: {
              id: 'user-1',
              email: 'athlete@example.com',
              displayName: 'Athlete',
              avatarUrl: null,
              roles: ['user']
            }
          }),
          {
            status: 200,
            headers: { 'content-type': 'application/json' }
          }
        )
      );

    global.fetch = fetchMock as typeof fetch;

    const result = await loadCurrentUser('');

    expect(fetchMock).toHaveBeenCalledWith('/api/auth/me', {
      method: 'GET',
      headers: { Accept: 'application/json' },
      credentials: 'include'
    });
    expect(result.authenticated).toBe(true);
    if (result.authenticated) {
      expect(result.user.email).toBe('athlete@example.com');
    }
  });
});
