import { useCallback, useEffect, useState } from 'react';
import { HashRouter, Route, Routes } from 'react-router-dom';

import { AppShell } from './app/AppShell';
import { getApiBaseUrl } from './config/env';
import { HomePage } from './pages/HomePage';
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
    <HashRouter>
      <Routes>
        <Route element={<AppShell backendStatus={backendStatus} />} path="/">
          <Route element={<HomePage backendStatus={backendStatus} />} index />
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
            path="settings"
          />
        </Route>
      </Routes>
    </HashRouter>
  );
}
