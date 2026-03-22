import { get, patch, AuthenticationError } from '../../../lib/httpClient';
import {
  userSettingsResponseSchema,
  updateAiAgentsRequestSchema,
  updateIntervalsRequestSchema,
  updateOptionsRequestSchema,
  updateCyclingRequestSchema,
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
  };
  return patch(apiBaseUrl, '/api/settings/ai-agents', trimmed);
}

export async function updateIntervals(apiBaseUrl: string, data: unknown) {
  const validated = updateIntervalsRequestSchema.parse(data);
  return patch(apiBaseUrl, '/api/settings/intervals', validated);
}

export async function updateOptions(apiBaseUrl: string, data: unknown) {
  const validated = updateOptionsRequestSchema.parse(data);
  return patch(apiBaseUrl, '/api/settings/options', validated);
}

export async function updateCycling(apiBaseUrl: string, data: unknown) {
  const validated = updateCyclingRequestSchema.parse(data);
  return patch(apiBaseUrl, '/api/settings/cycling', validated);
}
