import { render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { App } from './App';

const originalFetch = global.fetch;

describe('App', () => {
  beforeEach(() => {
    window.history.replaceState({}, '', '/');
  });

  afterEach(() => {
    global.fetch = originalFetch;
    vi.restoreAllMocks();
  });

  it('bootstraps auth and renders the public landing page for unauthenticated users', async () => {
    const fetchMock = vi.fn<(input: RequestInfo | URL, init?: RequestInit) => Promise<Response>>()
      .mockResolvedValueOnce(
        new Response(JSON.stringify({ authenticated: false }), {
          status: 200,
          headers: { 'content-type': 'application/json' }
        })
      )
      .mockResolvedValueOnce(
        new Response(JSON.stringify({ status: 'ok', service: 'AiWattCoach' }), {
          status: 200,
          headers: { 'content-type': 'application/json' }
        })
      )
      .mockResolvedValueOnce(
        new Response(JSON.stringify({ status: 'degraded', reason: 'mongo_unreachable' }), {
          status: 503,
          headers: { 'content-type': 'application/json' }
        })
      );

    global.fetch = fetchMock as typeof fetch;

    render(<App />);

    await waitFor(() => {
      expect(screen.getByRole('button', { name: /sign in with google/i })).toBeInTheDocument();
    });

    expect(fetchMock).toHaveBeenNthCalledWith(1, '/api/auth/me', {
      method: 'GET',
      headers: { Accept: 'application/json' },
      credentials: 'include'
    });
  });
});
