import { render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { AuthProvider, useAuth } from './AuthProvider';

const originalFetch = global.fetch;

function AuthProbe() {
  const auth = useAuth();

  return <div>{auth.status === 'authenticated' && auth.user ? auth.user.email : auth.status}</div>;
}

afterEach(() => {
  global.fetch = originalFetch;
  vi.restoreAllMocks();
});

describe('AuthProvider', () => {
  it('bootstraps the current user on mount', async () => {
    global.fetch = vi.fn<(input: RequestInfo | URL, init?: RequestInit) => Promise<Response>>()
      .mockResolvedValue(
        new Response(
          JSON.stringify({
            authenticated: true,
            user: {
              id: 'user-1',
              email: 'admin@example.com',
              displayName: 'Admin',
              avatarUrl: null,
              roles: ['user', 'admin']
            }
          }),
          {
            status: 200,
            headers: { 'content-type': 'application/json' }
          }
        )
      ) as typeof fetch;

    render(
      <AuthProvider apiBaseUrl="">
        <AuthProbe />
      </AuthProvider>
    );

    expect(screen.getByText(/loading/i)).toBeInTheDocument();

    await waitFor(() => {
      expect(screen.getByText('admin@example.com')).toBeInTheDocument();
    });
  });
});
