import { render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { AdminSystemInfoPage } from './AdminSystemInfoPage';

const originalFetch = global.fetch;

afterEach(() => {
  global.fetch = originalFetch;
  vi.restoreAllMocks();
});

describe('AdminSystemInfoPage', () => {
  it('renders the migrated backend diagnostics area', () => {
    global.fetch = vi.fn<(input: RequestInfo | URL, init?: RequestInit) => Promise<Response>>()
      .mockResolvedValue(
        new Response(JSON.stringify({ appName: 'AiWattCoach', mongoDatabase: 'aiwattcoach' }), {
          status: 200,
          headers: { 'content-type': 'application/json' }
        })
      ) as typeof fetch;

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
  });
});
