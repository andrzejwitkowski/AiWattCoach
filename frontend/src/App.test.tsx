import { render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { App } from './App';

const originalFetch = global.fetch;

describe('App', () => {
  beforeEach(() => {
    window.location.hash = '#/';
  });

  afterEach(() => {
    global.fetch = originalFetch;
    vi.restoreAllMocks();
  });

  it('shows loading first and then renders degraded backend state from the API', async () => {
    const fetchMock = vi.fn<(input: RequestInfo | URL, init?: RequestInit) => Promise<Response>>()
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

    expect(screen.getAllByText(/loading/i).length).toBeGreaterThan(0);

    await waitFor(() => {
      expect(screen.getAllByText(/degraded/i).length).toBeGreaterThan(0);
    });

    expect(screen.getAllByText(/backend status/i).length).toBeGreaterThan(0);
    expect(screen.getByText(/readiness/i)).toBeInTheDocument();
  });
});
