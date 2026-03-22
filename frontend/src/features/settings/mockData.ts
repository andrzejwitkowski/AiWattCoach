import type { UserSettingsResponse } from './types';

const twelveDaysAgo = Math.floor(Date.now() / 1000) - 12 * 24 * 60 * 60;

export const mockSettings: UserSettingsResponse = {
  aiAgents: {
    openaiApiKey: 'sk_test_REDACTED',
    openaiApiKeySet: true,
    geminiApiKey: '<redacted>',
    geminiApiKeySet: true,
  },
  intervals: {
    apiKey: '<redacted>',
    apiKeySet: true,
    athleteId: 'i123456',
    connected: true,
  },
  options: {
    analyzeWithoutHeartRate: true,
  },
  cycling: {
    fullName: 'Alex Rivier',
    age: 28,
    heightCm: 182,
    weightKg: 74,
    ftpWatts: 280,
    hrMaxBpm: 192,
    vo2Max: 62,
    lastZoneUpdateEpochSeconds: twelveDaysAgo,
  },
};
