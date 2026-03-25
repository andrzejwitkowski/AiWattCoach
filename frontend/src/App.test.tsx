import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { App } from './App';

const originalLocation = window.location;

const originalFetch = global.fetch;

describe('App', () => {
  beforeEach(() => {
    window.history.replaceState({}, '', '/');
  });

  afterEach(() => {
    global.fetch = originalFetch;
    Object.defineProperty(window, 'location', {
      configurable: true,
      value: originalLocation
    });
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
      headers: {
        Accept: 'application/json',
        traceparent: expect.stringMatching(/^[0-9a-f]{2}-[0-9a-f]{32}-[0-9a-f]{16}-[0-9a-f]{2}$/)
      },
      credentials: 'include'
    });
  });

  it('preserves the full deep link when starting Google login', async () => {
    window.history.replaceState({}, '', '/?returnTo=%2Fsettings%3Ftab%3Dsecurity%23billing');

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
        new Response(JSON.stringify({ status: 'ok', reason: null }), {
          status: 200,
          headers: { 'content-type': 'application/json' }
        })
      );
    global.fetch = fetchMock as typeof fetch;
    const assignMock = vi.fn();
    Object.defineProperty(window, 'location', {
      configurable: true,
      value: { ...window.location, assign: assignMock }
    });

    render(<App />);

    const signInButtons = await screen.findAllByRole('button', { name: /sign in with google/i });
    const signInButton = signInButtons.at(-1);
    expect(signInButton).toBeDefined();
    fireEvent.click(signInButton!);

    expect(assignMock).toHaveBeenCalledWith(
      '/api/auth/google/start?returnTo=%2Fsettings%3Ftab%3Dsecurity%23billing'
    );
  });
});
