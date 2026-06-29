import type { UserSettingsResponse } from './types';

export type TestSettingsOverrides = {
  aiAgents?: Partial<UserSettingsResponse['aiAgents']>;
  intervals?: Partial<UserSettingsResponse['intervals']>;
  wahoo?: Partial<UserSettingsResponse['wahoo']>;
  options?: Partial<UserSettingsResponse['options']>;
  availability?: Partial<UserSettingsResponse['availability']>;
  cycling?: Partial<UserSettingsResponse['cycling']>;
};

export const defaultAvailabilityDays: UserSettingsResponse['availability']['days'] = [
  { weekday: 'mon', available: true, maxDurationMinutes: 60 },
  { weekday: 'tue', available: false, maxDurationMinutes: null },
  { weekday: 'wed', available: true, maxDurationMinutes: 90 },
  { weekday: 'thu', available: false, maxDurationMinutes: null },
  { weekday: 'fri', available: true, maxDurationMinutes: 120 },
  { weekday: 'sat', available: false, maxDurationMinutes: null },
  { weekday: 'sun', available: false, maxDurationMinutes: null },
];

export function buildTestSettings(overrides: TestSettingsOverrides = {}): UserSettingsResponse {
  const settings: UserSettingsResponse = {
    aiAgents: {
      openaiApiKey: '***...1234',
      openaiApiKeySet: true,
      geminiApiKey: null,
      geminiApiKeySet: false,
      openrouterApiKey: '***...9999',
      openrouterApiKeySet: true,
      deepseekApiKey: null,
      deepseekApiKeySet: false,
      zaiApiKey: null,
      zaiApiKeySet: false,
      selectedProvider: 'openrouter',
      selectedModel: 'openai/gpt-4o-mini',
      workoutChatProvider: null,
      workoutChatModel: null,
      workoutPlanningProvider: null,
      workoutPlanningModel: null,
      mesoCycleProvider: null,
      mesoCycleModel: null,
    },
    intervals: {
      apiKey: null,
      apiKeySet: false,
      athleteId: null,
      connected: false,
    },
    wahoo: {
      available: false,
      accessToken: null,
      accessTokenSet: false,
      refreshTokenSet: false,
      expiresAtEpochSeconds: null,
      connected: false,
    },
    options: {
      analyzeWithoutHeartRate: false,
    },
    availability: {
      configured: true,
      days: defaultAvailabilityDays,
    },
    cycling: {
      fullName: null,
      age: null,
      heightCm: null,
      weightKg: null,
      ftpWatts: null,
      hrMaxBpm: null,
      vo2Max: null,
      athletePrompt: null,
      medications: null,
      athleteNotes: null,
      lastZoneUpdateEpochSeconds: null,
    },
  };

  if (overrides.aiAgents) {
    Object.assign(settings.aiAgents, overrides.aiAgents);
  }
  if (overrides.intervals) {
    Object.assign(settings.intervals, overrides.intervals);
  }
  if (overrides.wahoo) {
    Object.assign(settings.wahoo, overrides.wahoo);
  }
  if (overrides.options) {
    Object.assign(settings.options, overrides.options);
  }
  if (overrides.availability) {
    Object.assign(settings.availability, overrides.availability);
  }
  if (overrides.cycling) {
    Object.assign(settings.cycling, overrides.cycling);
  }

  return settings;
}

export function unsetAvailabilityDays(): UserSettingsResponse['availability']['days'] {
  return [
    { weekday: 'mon', available: false, maxDurationMinutes: null },
    { weekday: 'tue', available: false, maxDurationMinutes: null },
    { weekday: 'wed', available: false, maxDurationMinutes: null },
    { weekday: 'thu', available: false, maxDurationMinutes: null },
    { weekday: 'fri', available: false, maxDurationMinutes: null },
    { weekday: 'sat', available: false, maxDurationMinutes: null },
    { weekday: 'sun', available: false, maxDurationMinutes: null },
  ];
}