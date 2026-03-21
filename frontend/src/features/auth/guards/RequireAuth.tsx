import { Navigate, Outlet, useLocation } from 'react-router-dom';

import { useAuth } from '../context/AuthProvider';

export function RequireAuth() {
  const auth = useAuth();
  const location = useLocation();

  if (auth.status === 'loading') {
    return <div className="text-sm text-slate-300">Loading...</div>;
  }

  if (auth.status === 'unauthenticated') {
    return <Navigate replace state={{ from: location.pathname }} to="/" />;
  }

  return <Outlet />;
}
