import { get, patch, post, AuthenticationError } from '../../../lib/httpClient';
import {
  userSettingsResponseSchema,
  updateAiAgentsRequestSchema,
  updateIntervalsRequestSchema,
  updateOptionsRequestSchema,
  updateCyclingRequestSchema,
  testAiAgentsConnectionResponseSchema,
  testIntervalsConnectionResponseSchema,
} from '../types';

export async function loadSettings(apiBaseUrl: string) {
  try {
    const data = await get(apiBaseUrl, '/api/settings');
    return userSettingsResponseSchema.parse(data);
  } catch (err) {
    if (err instanceof AuthenticationError) {
      throw err;
    }
    throw new Error(`Failed to load settings: ${err instanceof Error ? err.message : String(err)}`);
  }
}

export async function updateAiAgents(apiBaseUrl: string, data: unknown) {
  const validated = updateAiAgentsRequestSchema.parse(data);
  const trimmed = {
    openaiApiKey: validated.openaiApiKey?.trim() ?? undefined,
    geminiApiKey: validated.geminiApiKey?.trim() ?? undefined,
    openrouterApiKey: validated.openrouterApiKey?.trim() ?? undefined,
    selectedProvider: validated.selectedProvider?.trim() ?? undefined,
    selectedModel: validated.selectedModel?.trim() ?? undefined,
  };
  return patch(apiBaseUrl, '/api/settings/ai-agents', trimmed);
}

export async function testAiAgentsConnection(apiBaseUrl: string, data: unknown) {
  const validated = updateAiAgentsRequestSchema.parse(data);
  const body = {
    openaiApiKey: validated.openaiApiKey?.trim() ?? undefined,
    geminiApiKey: validated.geminiApiKey?.trim() ?? undefined,
    openrouterApiKey: validated.openrouterApiKey?.trim() ?? undefined,
    selectedProvider: validated.selectedProvider?.trim() ?? undefined,
    selectedModel: validated.selectedModel?.trim() ?? undefined,
  };
  const parsed = await post<typeof body, unknown>(apiBaseUrl, '/api/settings/ai-agents/test', body, {
    allowStatuses: [400, 503],
  });
  return testAiAgentsConnectionResponseSchema.parse(parsed);
}

export async function updateIntervals(apiBaseUrl: string, data: unknown) {
  const validated = updateIntervalsRequestSchema.parse(data);
  const trimmed = {
    apiKey: validated.apiKey?.trim() || undefined,
    athleteId: validated.athleteId?.trim() || undefined,
  };
  return patch(apiBaseUrl, '/api/settings/intervals', trimmed);
}

export async function testIntervalsConnection(apiBaseUrl: string, data: unknown) {
  const validated = updateIntervalsRequestSchema.parse(data);
  const body = {
    apiKey: validated.apiKey?.trim() || undefined,
    athleteId: validated.athleteId?.trim() || undefined,
  };
  const parsed = await post<typeof body, unknown>(apiBaseUrl, '/api/settings/intervals/test', body, {
    allowStatuses: [400, 503],
  });
  return testIntervalsConnectionResponseSchema.parse(parsed);
}

export async function updateOptions(apiBaseUrl: string, data: unknown) {
  const validated = updateOptionsRequestSchema.parse(data);
  return patch(apiBaseUrl, '/api/settings/options', validated);
}

export async function updateCycling(apiBaseUrl: string, data: unknown) {
  const validated = updateCyclingRequestSchema.parse(data);
  return patch(apiBaseUrl, '/api/settings/cycling', validated);
}
