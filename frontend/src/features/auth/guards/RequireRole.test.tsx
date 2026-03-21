import { render, screen } from '@testing-library/react';
import { MemoryRouter, Route, Routes } from 'react-router-dom';
import { describe, expect, it } from 'vitest';

import { AuthContext, type AuthContextValue } from '../context/AuthProvider';
import { RequireRole } from './RequireRole';

function renderWithAuth(value: AuthContextValue) {
  render(
    <AuthContext.Provider value={value}>
      <MemoryRouter initialEntries={['/admin/system-info']}>
        <Routes>
          <Route element={<RequireRole role="admin" />}>
            <Route element={<div>Admin page</div>} path="/admin/system-info" />
          </Route>
          <Route element={<div>Forbidden page</div>} path="/app" />
        </Routes>
      </MemoryRouter>
    </AuthContext.Provider>
  );
}

describe('RequireRole', () => {
  it('redirects non-admin users away from admin routes', () => {
    renderWithAuth({
      status: 'authenticated',
      user: {
        id: 'user-1',
        email: 'athlete@example.com',
        displayName: 'Athlete',
        avatarUrl: null,
        roles: ['user']
      },
      refreshAuth: async () => {}
    });

    expect(screen.getByText(/forbidden page/i)).toBeInTheDocument();
  });
});
