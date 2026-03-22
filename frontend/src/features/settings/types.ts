export type AiAgentsSettings = {
  openaiApiKey: string | null;
  openaiApiKeySet: boolean;
  geminiApiKey: string | null;
  geminiApiKeySet: boolean;
};

export type IntervalsSettings = {
  apiKey: string | null;
  apiKeySet: boolean;
  athleteId: string | null;
  connected: boolean;
};

export type AnalysisOptionsSettings = {
  analyzeWithoutHeartRate: boolean;
};

export type CyclingSettingsData = {
  fullName: string | null;
  age: number | null;
  heightCm: number | null;
  weightKg: number | null;
  ftpWatts: number | null;
  hrMaxBpm: number | null;
  vo2Max: number | null;
  lastZoneUpdateEpochSeconds: number | null;
};

export type UserSettingsResponse = {
  aiAgents: AiAgentsSettings;
  intervals: IntervalsSettings;
  options: AnalysisOptionsSettings;
  cycling: CyclingSettingsData;
};

export type UpdateAiAgentsRequest = {
  openaiApiKey?: string | null;
  geminiApiKey?: string | null;
};

export type UpdateIntervalsRequest = {
  apiKey?: string | null;
  athleteId?: string | null;
};

export type UpdateOptionsRequest = {
  analyzeWithoutHeartRate?: boolean;
};

export type UpdateCyclingRequest = {
  fullName?: string | null;
  age?: number | null;
  heightCm?: number | null;
  weightKg?: number | null;
  ftpWatts?: number | null;
  hrMaxBpm?: number | null;
  vo2Max?: number | null;
};
