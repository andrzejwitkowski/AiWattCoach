import { render, screen } from '@testing-library/react';
import { MemoryRouter, Route, Routes } from 'react-router-dom';
import { describe, expect, it } from 'vitest';

import { AuthContext, type AuthContextValue } from '../context/AuthProvider';
import { RequireAuth } from './RequireAuth';

function renderWithAuth(value: AuthContextValue) {
  render(
    <AuthContext.Provider value={value}>
      <MemoryRouter initialEntries={['/settings']}>
        <Routes>
          <Route element={<RequireAuth />}>
            <Route element={<div>Protected page</div>} path="/settings" />
          </Route>
          <Route element={<div>Landing page</div>} path="/" />
        </Routes>
      </MemoryRouter>
    </AuthContext.Provider>
  );
}

describe('RequireAuth', () => {
  it('redirects unauthenticated users to the landing page', () => {
    renderWithAuth({ status: 'unauthenticated', user: null, refreshAuth: async () => {} });

    expect(screen.getByText(/landing page/i)).toBeInTheDocument();
  });
});
