import { Navigate, Outlet } from 'react-router-dom';

import { useAuth } from '../context/AuthProvider';
import type { AppRole } from '../types';

type RequireRoleProps = {
  role: AppRole;
};

export function RequireRole({ role }: RequireRoleProps) {
  const auth = useAuth();
  const currentUser = auth.user;

  if (auth.status === 'loading') {
    return <div className="text-sm text-slate-300">Loading...</div>;
  }

  if (auth.status !== 'authenticated' || !currentUser) {
    return <Navigate replace to="/" />;
  }

  if (!currentUser.roles.includes(role)) {
    return <Navigate replace to="/app" />;
  }

  return <Outlet />;
}
