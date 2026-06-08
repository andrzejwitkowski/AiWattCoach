import { get, patch, post, AuthenticationError } from '../../../lib/httpClient';
import {
  type UserSettingsResponse,
  userSettingsResponseSchema,
  updateAiAgentsRequestSchema,
  updateIntervalsRequestSchema,
  updateOptionsRequestSchema,
  updateAvailabilityRequestSchema,
  updateCyclingRequestSchema,
  testAiAgentsConnectionResponseSchema,
  testIntervalsConnectionResponseSchema,
} from '../types';
import { buildAiAgentsConnectionBody, getOptionalStringFieldValue } from './aiAgentsBody';
import { normalizeAiAgentsPayload } from './normalizeAiAgentsPayload';

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
  const validated = updateAiAgentsRequestSchema.parse(normalizeAiAgentsPayload(data));
  const body = buildAiAgentsConnectionBody(data, validated, { includeMesoFields: true });
  return patch(apiBaseUrl, '/api/settings/ai-agents', body);
}

export async function testAiAgentsConnection(apiBaseUrl: string, data: unknown) {
  const validated = updateAiAgentsRequestSchema.parse(normalizeAiAgentsPayload(data));
  const body = buildAiAgentsConnectionBody(data, validated);
  const parsed = await post<typeof body, unknown>(apiBaseUrl, '/api/settings/ai-agents/test', body, {
    allowedErrorStatuses: [400, 503],
  });
  return testAiAgentsConnectionResponseSchema.parse(parsed);
}

export async function updateIntervals(apiBaseUrl: string, data: unknown) {
  const validated = updateIntervalsRequestSchema.parse(data);
  const body: Record<string, string | null> = {};
  const apiKey = getOptionalStringFieldValue(data, 'apiKey', validated.apiKey);
  const athleteId = getOptionalStringFieldValue(data, 'athleteId', validated.athleteId);

  if (apiKey !== undefined) {
    body.apiKey = apiKey;
  }
  if (athleteId !== undefined) {
    body.athleteId = athleteId;
  }

  return patch(apiBaseUrl, '/api/settings/intervals', body);
}

export async function testIntervalsConnection(apiBaseUrl: string, data: unknown) {
  const validated = updateIntervalsRequestSchema.parse(data);
  const body = {
    apiKey: validated.apiKey?.trim() || undefined,
    athleteId: validated.athleteId?.trim() || undefined,
  };
  const parsed = await post<typeof body, unknown>(apiBaseUrl, '/api/settings/intervals/test', body, {
    allowedErrorStatuses: [400, 503],
  });
  return testIntervalsConnectionResponseSchema.parse(parsed);
}

export async function updateOptions(apiBaseUrl: string, data: unknown) {
  const validated = updateOptionsRequestSchema.parse(data);
  return patch(apiBaseUrl, '/api/settings/options', validated);
}

export async function updateAvailability(apiBaseUrl: string, data: unknown) {
  const validated = updateAvailabilityRequestSchema.parse(data);

  try {
    const parsed = await patch<typeof validated, unknown>(apiBaseUrl, '/api/settings/availability', validated, {
      allowedErrorStatuses: [400],
    });

    if (
      parsed
      && typeof parsed === 'object'
      && 'message' in parsed
      && typeof (parsed as { message?: unknown }).message === 'string'
    ) {
      throw new Error((parsed as { message: string }).message);
    }

    return userSettingsResponseSchema.parse(parsed) as UserSettingsResponse;
  } catch (error) {
    if (error instanceof AuthenticationError) {
      throw error;
    }

    throw new Error(`Failed to update availability: ${error instanceof Error ? error.message : String(error)}`);
  }
}

export async function updateCycling(apiBaseUrl: string, data: unknown) {
  const validated = updateCyclingRequestSchema.parse(data);
  const body = {
    ...validated,
    fullName: getOptionalStringFieldValue(data, 'fullName', validated.fullName),
    athletePrompt: getOptionalStringFieldValue(data, 'athletePrompt', validated.athletePrompt),
    medications: getOptionalStringFieldValue(data, 'medications', validated.medications),
    athleteNotes: getOptionalStringFieldValue(data, 'athleteNotes', validated.athleteNotes),
  };
  return patch(apiBaseUrl, '/api/settings/cycling', body);
}
