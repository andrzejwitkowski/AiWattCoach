import { useCallback, useEffect, useState } from 'react';
import { BrowserRouter, Route, Routes, useLocation, useNavigate } from 'react-router-dom';

import { AuthenticatedLayout } from './app/AuthenticatedLayout';
import { PublicLayout } from './app/PublicLayout';
import { getApiBaseUrl, isDevAuthEnabled } from './config/env';
import { buildGoogleLoginUrl, joinWhitelist } from './features/auth/api/auth';
import { AuthProvider } from './features/auth/context/AuthProvider';
import { RequireAuth } from './features/auth/guards/RequireAuth';
import { RequireRole } from './features/auth/guards/RequireRole';
import { SettingsProvider } from './features/settings/context/SettingsContext';
import { AppHomePage } from './pages/AppHomePage';
import { AdminSystemInfoPage } from './pages/AdminSystemInfoPage';
import { CalendarPage } from './pages/CalendarPage';
import { AICoachPage } from './pages/AICoachPage';
import { LandingPage } from './pages/LandingPage';
import { RacesPage } from './pages/RacesPage';
import { SettingsPage } from './pages/SettingsPage';

import { loadBackendStatus, type BackendStatus } from './lib/api/system';

const API_BASE_URL = getApiBaseUrl();
const DEV_AUTH_ENABLED = isDevAuthEnabled();
const API_BASE_URL_LABEL =
  API_BASE_URL || 'same-origin requests (Vite proxy in local development)';

const offlineFallback: BackendStatus = {
  health: {
    status: 'unknown',
    service: 'AiWattCoach',
  },
  readiness: {
    status: 'offline',
    reason: 'backend_unreachable',
  },
  state: 'offline',
  checkedAtLabel: 'not available',
};

const loadingFallback: BackendStatus = {
  health: {
    status: 'checking',
    service: 'AiWattCoach',
  },
  readiness: {
    status: 'checking',
    reason: 'checking_backend_status',
  },
  state: 'loading',
  checkedAtLabel: 'pending',
};

export const WHITELIST_REQUESTED_MESSAGE = 'Requested whitelist access. We will reach out after approval.';
export const PENDING_APPROVAL_MESSAGE = 'Your account is pending approval. Join the whitelist or wait for approval before signing in.';

export function App() {
  const [backendStatus, setBackendStatus] = useState<BackendStatus>(loadingFallback);
  const [isRefreshing, setIsRefreshing] = useState(true);

  const refreshBackendStatus = useCallback(async () => {
    setIsRefreshing(true);

    try {
      const status = await loadBackendStatus(API_BASE_URL);
      setBackendStatus(status);
    } catch {
      setBackendStatus(offlineFallback);
    } finally {
      setIsRefreshing(false);
    }
  }, []);

  useEffect(() => {
    let isActive = true;

    void (async () => {
      setIsRefreshing(true);

      try {
        const status = await loadBackendStatus(API_BASE_URL);
        if (!isActive) {
          return;
        }
        setBackendStatus(status);
      } catch {
        if (!isActive) {
          return;
        }
        setBackendStatus(offlineFallback);
      } finally {
        if (isActive) {
          setIsRefreshing(false);
        }
      }
    })();

    return () => {
      isActive = false;
    };
  }, []);

  return (
    <AuthProvider apiBaseUrl={API_BASE_URL}>
      <BrowserRouter>
        <Routes>
          <Route element={<PublicLayout />}>
            <Route element={<PublicLandingRoute apiBaseUrl={API_BASE_URL} />} path="/" />
          </Route>

          <Route element={<RequireAuth />}>
            <Route
              element={
                <SettingsProvider apiBaseUrl={API_BASE_URL}>
                  <AuthenticatedLayout apiBaseUrl={API_BASE_URL} backendStatus={backendStatus} />
                </SettingsProvider>
              }
            >
              <Route element={<AppHomePage apiBaseUrl={API_BASE_URL} />} path="/app" />
              <Route element={<SettingsPage apiBaseUrl={API_BASE_URL} />} path="/settings" />
              <Route element={<CalendarPage apiBaseUrl={API_BASE_URL} />} path="/calendar" />
              <Route element={<RacesPage apiBaseUrl={API_BASE_URL} />} path="/races" />
              <Route element={<AICoachPage apiBaseUrl={API_BASE_URL} />} path="/ai-coach" />
              <Route element={<RequireRole role="admin" />}>
                <Route
                  element={
                    <AdminSystemInfoPage
                      apiBaseUrl={API_BASE_URL}
                      apiBaseUrlLabel={API_BASE_URL_LABEL}
                      backendStatus={backendStatus}
                      isRefreshing={isRefreshing}
                      onRefresh={() => {
                        void refreshBackendStatus();
                      }}
                    />
                  }
                  path="/admin/system-info"
                />
              </Route>
            </Route>
          </Route>
        </Routes>
      </BrowserRouter>
    </AuthProvider>
  );
}

function PublicLandingRoute({ apiBaseUrl }: { apiBaseUrl: string }) {
  const location = useLocation();
  const navigate = useNavigate();
  const searchParams = new URLSearchParams(location.search);
  const searchReturnTo = searchParams.get('returnTo');
  const authStatus = searchParams.get('auth');
  const whitelistStatus = searchParams.get('whitelist');
  const stateValue = (location.state as { from?: unknown } | null)?.from;
  const stateReturnTo = typeof stateValue === 'string' && stateValue.length > 0 ? stateValue : null;
  const returnTo = (typeof searchReturnTo === 'string' && searchReturnTo.length > 0 ? searchReturnTo : null) || stateReturnTo || '/calendar';
  const whitelistMessage = whitelistStatus === 'requested' ? WHITELIST_REQUESTED_MESSAGE : null;
  const authMessage = authStatus === 'pending-approval' ? PENDING_APPROVAL_MESSAGE : null;

  return (
    <LandingPage
      devAuthEnabled={DEV_AUTH_ENABLED}
      authMessage={authMessage}
      whitelistMessage={whitelistMessage}
      onLogin={() => {
        window.location.assign(buildGoogleLoginUrl(apiBaseUrl, returnTo));
      }}
      onJoinWhitelist={async (email) => {
        await joinWhitelist(apiBaseUrl, email);
        const params = new URLSearchParams(location.search);
        params.delete('auth');
        params.set('whitelist', 'requested');
        void navigate({
          pathname: location.pathname,
          search: `?${params.toString()}`
        }, {
          replace: true,
          state: location.state
        });
      }}
    />
  );
}
