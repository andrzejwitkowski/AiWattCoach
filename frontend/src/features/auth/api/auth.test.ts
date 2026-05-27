import { afterEach, describe, expect, it, vi } from 'vitest';

import { buildGoogleLoginUrl, buildWahooConnectUrl, joinWhitelist, loadCurrentUser } from './auth';

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
      headers: {
        Accept: 'application/json',
        traceparent: expect.stringMatching(/^[0-9a-f]{2}-[0-9a-f]{32}-[0-9a-f]{16}-[0-9a-f]{2}$/)
      },
      credentials: 'include'
    });
    expect(result.authenticated).toBe(true);
    if (result.authenticated) {
      expect(result.user.email).toBe('athlete@example.com');
    }
  });

  it('rejects malformed auth payloads', async () => {
    const fetchMock = vi.fn<(input: RequestInfo | URL, init?: RequestInit) => Promise<Response>>()
      .mockResolvedValue(
        new Response(JSON.stringify({ authenticated: true }), {
          status: 200,
          headers: { 'content-type': 'application/json' }
        })
      );

    global.fetch = fetchMock as typeof fetch;

    await expect(loadCurrentUser('')).rejects.toThrow();
  });
});

describe('buildGoogleLoginUrl', () => {
  it('defaults returnTo to the calendar page', () => {
    expect(buildGoogleLoginUrl('')).toBe('/api/auth/google/start?returnTo=%2Fcalendar');
  });
});

describe('buildWahooConnectUrl', () => {
  it('defaults returnTo to the settings page', () => {
    expect(buildWahooConnectUrl('')).toBe('/api/auth/wahoo/start?returnTo=%2Fsettings');
  });
});

describe('joinWhitelist', () => {
  it('posts email to whitelist endpoint', async () => {
    const fetchMock = vi.fn<(input: RequestInfo | URL, init?: RequestInit) => Promise<Response>>()
      .mockResolvedValue(
        new Response(JSON.stringify({ success: true }), {
          status: 200,
          headers: { 'content-type': 'application/json' }
        })
      );

    global.fetch = fetchMock as typeof fetch;

    const result = await joinWhitelist('', 'athlete@example.com');

    expect(fetchMock).toHaveBeenCalledWith('/api/auth/whitelist', {
      method: 'POST',
      credentials: 'include',
      headers: {
        Accept: 'application/json',
        'Content-Type': 'application/json',
        traceparent: expect.stringMatching(/^[0-9a-f]{2}-[0-9a-f]{32}-[0-9a-f]{16}-[0-9a-f]{2}$/)
      },
      body: JSON.stringify({ email: 'athlete@example.com' })
    });
    expect(result.success).toBe(true);
  });
});
