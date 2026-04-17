import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { App, PENDING_APPROVAL_MESSAGE, WHITELIST_REQUESTED_MESSAGE } from './App';

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
    expect(screen.getByRole('button', { name: /join whitelist/i })).toBeInTheDocument();

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

  it('defaults login redirect to the calendar page', async () => {
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
    fireEvent.click(signInButtons.at(-1)!);

    expect(assignMock).toHaveBeenCalledWith('/api/auth/google/start?returnTo=%2Fcalendar');
  });

  it('shows pending approval message from auth query param', async () => {
    window.history.replaceState({}, '', '/?auth=pending-approval');

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

    render(<App />);

    await waitFor(() => {
      expect(screen.getByText(PENDING_APPROVAL_MESSAGE)).toBeInTheDocument();
    });
  });

  it('submits whitelist request and shows success message', async () => {
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
      )
      .mockResolvedValueOnce(
        new Response(JSON.stringify({ success: true }), {
          status: 200,
          headers: { 'content-type': 'application/json' }
        })
      );

    global.fetch = fetchMock as typeof fetch;

    render(<App />);

    const inputs = await screen.findAllByPlaceholderText(/you@example.com/i);
    const input = inputs.at(-1);
    expect(input).toBeDefined();
    fireEvent.change(input!, { target: { value: 'athlete@example.com' } });
    const buttons = screen.getAllByRole('button', { name: /join whitelist/i });
    fireEvent.click(buttons.at(-1)!);

    await waitFor(() => {
      expect(screen.getByText(WHITELIST_REQUESTED_MESSAGE)).toBeInTheDocument();
    });

    expect(fetchMock).toHaveBeenNthCalledWith(4, '/api/auth/whitelist', {
      method: 'POST',
      credentials: 'include',
      headers: {
        Accept: 'application/json',
        'Content-Type': 'application/json',
        traceparent: expect.stringMatching(/^[0-9a-f]{2}-[0-9a-f]{32}-[0-9a-f]{16}-[0-9a-f]{2}$/)
      },
      body: JSON.stringify({ email: 'athlete@example.com' })
    });
  });

  it('keeps the protected deep link after whitelist submission from a RequireAuth redirect', async () => {
    window.history.replaceState({}, '', '/settings?tab=security');

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
      )
      .mockResolvedValueOnce(
        new Response(JSON.stringify({ success: true }), {
          status: 200,
          headers: { 'content-type': 'application/json' }
        })
      );

    global.fetch = fetchMock as typeof fetch;

    render(<App />);

    await waitFor(() => {
      expect(window.location.pathname).toBe('/');
    });

    const input = (await screen.findAllByPlaceholderText(/you@example.com/i)).at(-1);
    expect(input).toBeDefined();
    fireEvent.change(input!, { target: { value: 'athlete@example.com' } });
    fireEvent.click(screen.getAllByRole('button', { name: /join whitelist/i }).at(-1)!);

    await waitFor(() => {
      expect(screen.getByText(WHITELIST_REQUESTED_MESSAGE)).toBeInTheDocument();
    });

    const assignMock = vi.fn();
    Object.defineProperty(window, 'location', {
      configurable: true,
      value: { ...window.location, assign: assignMock }
    });

    fireEvent.click(screen.getAllByRole('button', { name: /sign in with google/i }).at(-1)!);

    expect(assignMock).toHaveBeenCalledWith(
      '/api/auth/google/start?returnTo=%2Fsettings%3Ftab%3Dsecurity'
    );
  });
});
