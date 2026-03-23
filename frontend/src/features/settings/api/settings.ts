import { get, patch, AuthenticationError, HttpError, buildUrl } from '../../../lib/httpClient';
import {
  userSettingsResponseSchema,
  updateAiAgentsRequestSchema,
  updateIntervalsRequestSchema,
  updateOptionsRequestSchema,
  updateCyclingRequestSchema,
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
  };
  return patch(apiBaseUrl, '/api/settings/ai-agents', trimmed);
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
  const response = await fetch(buildUrl(apiBaseUrl, '/api/settings/intervals/test'), {
    method: 'POST',
    headers: {
      Accept: 'application/json',
      'Content-Type': 'application/json',
    },
    credentials: 'include',
    body: JSON.stringify(body),
  });

  if (response.status === 401) {
    throw new AuthenticationError();
  }

  if (![200, 400, 503].includes(response.status)) {
    throw new HttpError(response.status, `POST /api/settings/intervals/test failed: ${response.status}`);
  }

  let parsed: unknown;
  try {
    parsed = await response.json();
  } catch {
    throw new HttpError(response.status, 'POST /api/settings/intervals/test: invalid JSON response');
  }

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
