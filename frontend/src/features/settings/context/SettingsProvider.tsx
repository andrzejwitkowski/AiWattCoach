import { createContext, useCallback, useContext, useState, type ReactNode } from 'react';
import type {
  UserSettingsResponse,
  UpdateAiAgentsRequest,
  UpdateIntervalsRequest,
  UpdateOptionsRequest,
  UpdateCyclingRequest,
} from '../types';
import { mockSettings } from '../mockData';

type SettingsContextValue = {
  settings: UserSettingsResponse;
  isLoading: boolean;
  isSaving: Record<string, boolean>;
  updateAiAgents: (data: UpdateAiAgentsRequest) => Promise<void>;
  updateIntervals: (data: UpdateIntervalsRequest) => Promise<void>;
  updateOptions: (data: UpdateOptionsRequest) => Promise<void>;
  updateCycling: (data: UpdateCyclingRequest) => Promise<void>;
};

const SettingsContext = createContext<SettingsContextValue | null>(null);

export function SettingsProvider({ children }: { children: ReactNode }) {
  const [settings, setSettings] = useState<UserSettingsResponse>(mockSettings);
  const [isSaving, setIsSaving] = useState<Record<string, boolean>>({});

  const selectedProvider = (value: UpdateAiAgentsRequest['selectedProvider']) => value || null;
  const selectedModel = (value: UpdateAiAgentsRequest['selectedModel']) => value || null;

  const updateAiAgents = useCallback(async (data: UpdateAiAgentsRequest) => {
    setIsSaving((prev) => ({ ...prev, aiAgents: true }));
    try {
      setSettings((prev) => ({
        ...prev,
        aiAgents: {
          ...prev.aiAgents,
          ...(data.openaiApiKey !== undefined && {
            openaiApiKey: data.openaiApiKey ?? null,
            openaiApiKeySet: data.openaiApiKey != null,
          }),
          ...(data.geminiApiKey !== undefined && {
            geminiApiKey: data.geminiApiKey ?? null,
            geminiApiKeySet: data.geminiApiKey != null,
          }),
          ...(data.openrouterApiKey !== undefined && {
            openrouterApiKey: data.openrouterApiKey ?? null,
            openrouterApiKeySet: data.openrouterApiKey != null,
          }),
          ...(data.selectedProvider !== undefined && {
            selectedProvider: selectedProvider(data.selectedProvider),
          }),
          ...(data.selectedModel !== undefined && {
            selectedModel: selectedModel(data.selectedModel),
          }),
        },
      }));
    } finally {
      setIsSaving((prev) => ({ ...prev, aiAgents: false }));
    }
  }, []);

  const updateIntervals = useCallback(async (data: UpdateIntervalsRequest) => {
    setIsSaving((prev) => ({ ...prev, intervals: true }));
    try {
      setSettings((prev) => ({
        ...prev,
        intervals: {
          ...prev.intervals,
          ...(data.apiKey !== undefined && {
            apiKey: data.apiKey ?? null,
            apiKeySet: data.apiKey != null,
          }),
          ...(data.athleteId !== undefined && {
            athleteId: data.athleteId ?? null,
          }),
        },
      }));
    } finally {
      setIsSaving((prev) => ({ ...prev, intervals: false }));
    }
  }, []);

  const updateOptions = useCallback(async (data: UpdateOptionsRequest) => {
    setIsSaving((prev) => ({ ...prev, options: true }));
    try {
      setSettings((prev) => ({
        ...prev,
        options: {
          ...prev.options,
          ...(data.analyzeWithoutHeartRate !== undefined && {
            analyzeWithoutHeartRate: data.analyzeWithoutHeartRate,
          }),
        },
      }));
    } finally {
      setIsSaving((prev) => ({ ...prev, options: false }));
    }
  }, []);

  const updateCycling = useCallback(async (data: UpdateCyclingRequest) => {
    setIsSaving((prev) => ({ ...prev, cycling: true }));
    try {
      setSettings((prev) => ({
        ...prev,
        cycling: {
          ...prev.cycling,
          ...(data.fullName !== undefined && { fullName: data.fullName ?? null }),
          ...(data.age !== undefined && { age: data.age ?? null }),
          ...(data.heightCm !== undefined && { heightCm: data.heightCm ?? null }),
          ...(data.weightKg !== undefined && { weightKg: data.weightKg ?? null }),
          ...(data.ftpWatts !== undefined && { ftpWatts: data.ftpWatts ?? null }),
          ...(data.hrMaxBpm !== undefined && { hrMaxBpm: data.hrMaxBpm ?? null }),
          ...(data.vo2Max !== undefined && { vo2Max: data.vo2Max ?? null }),
        },
      }));
    } finally {
      setIsSaving((prev) => ({ ...prev, cycling: false }));
    }
  }, []);

  return (
    <SettingsContext.Provider
      value={{
        settings,
        isLoading: false,
        isSaving,
        updateAiAgents,
        updateIntervals,
        updateOptions,
        updateCycling,
      }}
    >
      {children}
    </SettingsContext.Provider>
  );
}

export function useSettings(): SettingsContextValue {
  const ctx = useContext(SettingsContext);
  if (!ctx) {
    throw new Error('useSettings must be used within a SettingsProvider');
  }
  return ctx;
}
