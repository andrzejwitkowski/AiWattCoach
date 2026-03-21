import { useCallback, useEffect, useState } from 'react';
import { BrowserRouter, Route, Routes } from 'react-router-dom';

import { AuthenticatedLayout } from './app/AuthenticatedLayout';
import { PublicLayout } from './app/PublicLayout';
import { getApiBaseUrl } from './config/env';
import { buildGoogleLoginUrl } from './features/auth/api/auth';
import { AuthProvider } from './features/auth/context/AuthProvider';
import { RequireAuth } from './features/auth/guards/RequireAuth';
import { RequireRole } from './features/auth/guards/RequireRole';
import { AppHomePage } from './pages/AppHomePage';
import { AdminSystemInfoPage } from './pages/AdminSystemInfoPage';
import { LandingPage } from './pages/LandingPage';
import { SettingsPage } from './pages/SettingsPage';
import { loadBackendStatus, type BackendStatus } from './lib/api/system';

const API_BASE_URL = getApiBaseUrl();
const API_BASE_URL_LABEL = API_BASE_URL || 'same-origin requests (Vite proxy in local development)';

const offlineFallback: BackendStatus = {
  health: {
    status: 'unknown',
    service: 'AiWattCoach'
  },
  readiness: {
    status: 'offline',
    reason: 'backend_unreachable'
  },
  state: 'offline',
  checkedAtLabel: 'not available'
};

const loadingFallback: BackendStatus = {
  health: {
    status: 'checking',
    service: 'AiWattCoach'
  },
  readiness: {
    status: 'checking',
    reason: 'checking_backend_status'
  },
  state: 'loading',
  checkedAtLabel: 'pending'
};

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
    void refreshBackendStatus();
  }, [refreshBackendStatus]);

  return (
    <AuthProvider apiBaseUrl={API_BASE_URL}>
      <BrowserRouter>
        <Routes>
          <Route element={<PublicLayout />}>
            <Route
              element={
                <LandingPage
                  onLogin={() => {
                    window.location.assign(buildGoogleLoginUrl(API_BASE_URL));
                  }}
                />
              }
              path="/"
            />
          </Route>

          <Route element={<RequireAuth />}>
            <Route
              element={<AuthenticatedLayout apiBaseUrl={API_BASE_URL} backendStatus={backendStatus} />}
            >
              <Route element={<AppHomePage />} path="/app" />
              <Route
                element={
                  <SettingsPage
                    apiBaseUrlLabel={API_BASE_URL_LABEL}
                    backendStatus={backendStatus}
                    isRefreshing={isRefreshing}
                    onRefresh={() => {
                      void refreshBackendStatus();
                    }}
                  />
                }
                path="/settings"
              />
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
