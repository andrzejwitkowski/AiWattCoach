import { describe, expect, it } from 'vitest';

import { buildTestSettings } from '../../settings/mockData';
import type { UserSettingsResponse } from '../../settings/types';
import { canGenerateMesoCycle } from './useMesoCycleCalendar';

function buildSettings(
  overrides?: Partial<UserSettingsResponse['aiAgents']>,
): UserSettingsResponse {
  return buildTestSettings({
    aiAgents: {
      openaiApiKey: null,
      openaiApiKeySet: true,
      geminiApiKey: null,
      geminiApiKeySet: false,
      openrouterApiKey: null,
      openrouterApiKeySet: false,
      deepseekApiKey: null,
      deepseekApiKeySet: false,
      zaiApiKey: null,
      zaiApiKeySet: false,
      selectedProvider: 'openai',
      selectedModel: 'gpt-5',
      mesoCycleProvider: null,
      mesoCycleModel: null,
      ...overrides,
    },
    intervals: {
      apiKey: null,
      apiKeySet: true,
      athleteId: 'i123',
      connected: true,
    },
  });
}

describe('canGenerateMesoCycle', () => {
  it('allows generation when meso-specific provider overrides are configured', () => {
    expect(
      canGenerateMesoCycle(
        buildSettings({
          mesoCycleProvider: 'gemini',
          mesoCycleModel: 'gemini-2.5-flash',
          geminiApiKeySet: true,
          selectedProvider: null,
          selectedModel: null,
          openaiApiKeySet: false,
        }),
      ),
    ).toBe(true);
  });

  it('blocks generation when no provider or model is configured', () => {
    expect(
      canGenerateMesoCycle(
        buildSettings({
          selectedProvider: null,
          selectedModel: null,
        }),
      ),
    ).toBe(false);
  });
});
