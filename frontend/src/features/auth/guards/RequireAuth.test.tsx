import { render, screen } from '@testing-library/react';
import { MemoryRouter, Route, Routes, useLocation } from 'react-router-dom';
import { describe, expect, it } from 'vitest';

import { AuthContext, type AuthContextValue } from '../context/AuthProvider';
import { RequireAuth } from './RequireAuth';

function renderWithAuth(value: AuthContextValue) {
  render(
    <AuthContext.Provider value={value}>
      <MemoryRouter initialEntries={['/settings?tab=security#billing']}>
        <Routes>
          <Route element={<RequireAuth />}>
            <Route element={<div>Protected page</div>} path="/settings" />
          </Route>
          <Route element={<LandingPageState />} path="/" />
        </Routes>
      </MemoryRouter>
    </AuthContext.Provider>
  );
}

function LandingPageState() {
  const location = useLocation();
  return (
    <div>
      <div>Landing page</div>
      <div data-testid="redirect-from">{(location.state as { from?: string } | null)?.from ?? 'none'}</div>
    </div>
  );
}

describe('RequireAuth', () => {
  it('redirects unauthenticated users to the landing page', () => {
    renderWithAuth({ status: 'unauthenticated', user: null, refreshAuth: async () => {} });

    expect(screen.getByText(/landing page/i)).toBeInTheDocument();
    expect(screen.getByTestId('redirect-from')).toHaveTextContent('/settings?tab=security#billing');
  });
});
