import { render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { AdminSystemInfoPage } from './AdminSystemInfoPage';

const originalFetch = global.fetch;

afterEach(() => {
  global.fetch = originalFetch;
  vi.restoreAllMocks();
});

describe('AdminSystemInfoPage', () => {
  it('renders the migrated backend diagnostics area', async () => {
    const fetchMock = vi.fn<(input: RequestInfo | URL, init?: RequestInit) => Promise<Response>>()
      .mockResolvedValue(
        new Response(JSON.stringify({ appName: 'AiWattCoach', mongoDatabase: 'aiwattcoach' }), {
          status: 200,
          headers: { 'content-type': 'application/json' }
        })
      );

    global.fetch = fetchMock as typeof fetch;

    render(
      <AdminSystemInfoPage
        apiBaseUrl=""
        apiBaseUrlLabel="same-origin"
        backendStatus={{
          health: { status: 'ok', service: 'AiWattCoach' },
          readiness: { status: 'ok', reason: null },
          state: 'online',
          checkedAtLabel: 'just now'
        }}
        isRefreshing={false}
        onRefresh={() => {}}
      />
    );

    expect(screen.getByRole('heading', { name: /operational diagnostics for the admin workspace/i })).toBeInTheDocument();
    expect(screen.getByText(/backend status/i)).toBeInTheDocument();
    await waitFor(() => {
      expect(fetchMock).toHaveBeenCalledWith('/api/admin/system-info', {
        method: 'GET',
        headers: {
          Accept: 'application/json',
          traceparent: expect.stringMatching(/^[0-9a-f]{2}-[0-9a-f]{32}-[0-9a-f]{16}-[0-9a-f]{2}$/)
        },
        credentials: 'include'
      });
    });
    expect(await screen.findByText('Admin-only payload')).toBeInTheDocument();
    await waitFor(() => {
      expect(screen.getAllByText('AiWattCoach')).toHaveLength(2);
      expect(screen.getByText('aiwattcoach')).toBeInTheDocument();
    });
  });
});
