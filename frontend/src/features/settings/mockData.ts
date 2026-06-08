import type { UserSettingsResponse } from './types';

export const defaultAvailabilityDays: UserSettingsResponse['availability']['days'] = [
  { weekday: 'mon', available: true, maxDurationMinutes: 60 },
  { weekday: 'tue', available: false, maxDurationMinutes: null },
  { weekday: 'wed', available: true, maxDurationMinutes: 90 },
  { weekday: 'thu', available: false, maxDurationMinutes: null },
  { weekday: 'fri', available: true, maxDurationMinutes: 120 },
  { weekday: 'sat', available: false, maxDurationMinutes: null },
  { weekday: 'sun', available: false, maxDurationMinutes: null },
];

export function buildTestSettings(
  overrides?: Partial<UserSettingsResponse> & { aiAgents?: Partial<UserSettingsResponse['aiAgents']> },
): UserSettingsResponse {
  const { aiAgents: aiAgentsOverrides, ...rest } = overrides ?? {};

  return {
    aiAgents: {
      openaiApiKey: '***...1234',
      openaiApiKeySet: true,
      geminiApiKey: null,
      geminiApiKeySet: false,
      openrouterApiKey: '***...9999',
      openrouterApiKeySet: true,
      deepseekApiKey: null,
      deepseekApiKeySet: false,
      selectedProvider: 'openrouter',
      selectedModel: 'openai/gpt-4o-mini',
      mesoCycleProvider: null,
      mesoCycleModel: null,
      ...aiAgentsOverrides,
    },
    intervals: {
      apiKey: null,
      apiKeySet: false,
      athleteId: null,
      connected: false,
      ...rest.intervals,
    },
    wahoo: {
      available: false,
      accessToken: null,
      accessTokenSet: false,
      refreshTokenSet: false,
      expiresAtEpochSeconds: null,
      connected: false,
      ...rest.wahoo,
    },
    options: {
      analyzeWithoutHeartRate: false,
      ...rest.options,
    },
    availability: {
      configured: true,
      days: defaultAvailabilityDays,
      ...rest.availability,
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
      ...rest.cycling,
    },
    ...rest,
  };
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

export function settingsApiResponseBody(
  overrides?: Partial<UserSettingsResponse> & { aiAgents?: Partial<UserSettingsResponse['aiAgents']> },
): UserSettingsResponse {
  return buildTestSettings(overrides);
}
