import { createContext, useCallback, useContext, useEffect, useState } from 'react';
import { loadSettings } from '../api/settings';
import type { UserSettingsResponse } from '../types';

type SettingsContextValue = {
  settings: UserSettingsResponse | null;
  isLoading: boolean;
  error: string | null;
  refreshSettings: () => Promise<void>;
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
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const refreshSettings = useCallback(async () => {
    setIsLoading(true);
    setError(null);
    try {
      const data = await loadSettings(apiBaseUrl);
      setSettings(data);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to load settings');
    } finally {
      setIsLoading(false);
    }
  }, [apiBaseUrl]);

  useEffect(() => {
    void refreshSettings();
  }, [refreshSettings]);

  return (
    <SettingsContext.Provider value={{ settings, isLoading, error, refreshSettings }}>
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
