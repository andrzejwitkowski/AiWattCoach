import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
  type PropsWithChildren
} from 'react';

import { loadCurrentUser } from '../api/auth';
import type { AuthStatus, CurrentUser } from '../types';

export type AuthContextValue = {
  status: AuthStatus;
  user: CurrentUser | null;
  refreshAuth: () => Promise<void>;
};

export const AuthContext = createContext<AuthContextValue | null>(null);

type AuthProviderProps = PropsWithChildren<{
  apiBaseUrl: string;
}>;

export function AuthProvider({ apiBaseUrl, children }: AuthProviderProps) {
  const [status, setStatus] = useState<AuthStatus>('loading');
  const [user, setUser] = useState<CurrentUser | null>(null);

  const refreshAuth = useCallback(async () => {
    setStatus('loading');

    try {
      const response = await loadCurrentUser(apiBaseUrl);

      if (response.authenticated) {
        setUser(response.user);
        setStatus('authenticated');
        return;
      }

      setUser(null);
      setStatus('unauthenticated');
    } catch {
      setUser(null);
      setStatus('unauthenticated');
    }
  }, [apiBaseUrl]);

  useEffect(() => {
    let isActive = true;

    void (async () => {
      setStatus('loading');

      try {
        const response = await loadCurrentUser(apiBaseUrl);

        if (!isActive) {
          return;
        }

        if (response.authenticated) {
          setUser(response.user);
          setStatus('authenticated');
          return;
        }

        setUser(null);
        setStatus('unauthenticated');
      } catch {
        if (!isActive) {
          return;
        }

        setUser(null);
        setStatus('unauthenticated');
      }
    })();

    return () => {
      isActive = false;
    };
  }, [apiBaseUrl]);

  const value = useMemo(
    () => ({
      status,
      user,
      refreshAuth
    }),
    [refreshAuth, status, user]
  );

  return <AuthContext.Provider value={value}>{children}</AuthContext.Provider>;
}

export function useAuth() {
  const context = useContext(AuthContext);

  if (!context) {
    throw new Error('useAuth must be used within AuthProvider');
  }

  return context;
}
