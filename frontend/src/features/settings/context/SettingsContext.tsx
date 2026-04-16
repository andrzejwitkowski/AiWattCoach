import { createContext, useCallback, useContext, useEffect, useState } from 'react';
import { loadSettings } from '../api/settings';
import { AuthenticationError } from '../../../lib/httpClient';
import type { UserSettingsResponse } from '../types';

type SettingsContextValue = {
  settings: UserSettingsResponse | null;
  isLoading: boolean;
  error: string | null;
  refreshSettings: (options?: { background?: boolean }) => Promise<void>;
  setSettings: React.Dispatch<React.SetStateAction<UserSettingsResponse | null>>;
};

const SettingsContext = createContext<SettingsContextValue | null>(null);

export function SettingsProvider({
  children,
  apiBaseUrl,
}: {
  children: React.ReactNode;
  apiBaseUrl: string;
}) {
  const [settings, setSettings] = useState<UserSettingsResponse | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const refreshSettings = useCallback(async (options?: { background?: boolean }) => {
    const background = options?.background ?? false;
    if (!background) {
      setIsLoading(true);
    }
    setError(null);
    try {
      const data = await loadSettings(apiBaseUrl);
      setSettings(data);
    } catch (err) {
      if (err instanceof AuthenticationError) {
        window.location.href = '/';
        return;
      }
      setError(err instanceof Error ? err.message : 'Failed to load settings');
    } finally {
      setIsLoading(false);
    }
  }, [apiBaseUrl]);

  useEffect(() => {
    void refreshSettings();
  }, [refreshSettings]);

  return (
    <SettingsContext.Provider value={{ settings, isLoading, error, refreshSettings, setSettings }}>
      {children}
    </SettingsContext.Provider>
  );
}

export function useSettings() {
  const context = useContext(SettingsContext);
  if (!context) {
    throw new Error('useSettings must be used within a SettingsProvider');
  }
  return context;
}
