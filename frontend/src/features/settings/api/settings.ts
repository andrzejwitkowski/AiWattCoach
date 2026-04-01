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

function trimToUndefined(value: string | null | undefined) {
  const trimmed = value?.trim();
  return trimmed ? trimmed : undefined;
}

function getSelectionPatchValue(
  data: unknown,
  key: 'selectedProvider' | 'selectedModel',
  validatedValue: string | null | undefined,
) {
  if (!data || typeof data !== 'object' || !(key in data)) {
    return undefined;
  }

  const rawValue = (data as Record<string, unknown>)[key];
  if (rawValue === '' || rawValue === null) {
    return '';
  }

  return trimToUndefined(validatedValue);
}

function normalizeAiAgentsPayload(data: unknown) {
  if (!data || typeof data !== 'object') {
    return data;
  }

  const candidate = data as Record<string, unknown>;
  return {
    ...candidate,
    openaiApiKey:
      typeof candidate.openaiApiKey === 'string' ? candidate.openaiApiKey.trim() : candidate.openaiApiKey,
    geminiApiKey:
      typeof candidate.geminiApiKey === 'string' ? candidate.geminiApiKey.trim() : candidate.geminiApiKey,
    openrouterApiKey:
      typeof candidate.openrouterApiKey === 'string'
        ? candidate.openrouterApiKey.trim()
        : candidate.openrouterApiKey,
    selectedProvider:
      typeof candidate.selectedProvider === 'string'
        ? candidate.selectedProvider.trim()
        : candidate.selectedProvider,
    selectedModel:
      typeof candidate.selectedModel === 'string' ? candidate.selectedModel.trim() : candidate.selectedModel,
  };
}

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
  const trimmed: Record<string, string> = {};

  const openaiApiKey = trimToUndefined(validated.openaiApiKey);
  const geminiApiKey = trimToUndefined(validated.geminiApiKey);
  const openrouterApiKey = trimToUndefined(validated.openrouterApiKey);

  if (openaiApiKey !== undefined) {
    trimmed.openaiApiKey = openaiApiKey;
  }
  if (geminiApiKey !== undefined) {
    trimmed.geminiApiKey = geminiApiKey;
  }
  if (openrouterApiKey !== undefined) {
    trimmed.openrouterApiKey = openrouterApiKey;
  }
  const selectedProvider = getSelectionPatchValue(data, 'selectedProvider', validated.selectedProvider);
  const selectedModel = getSelectionPatchValue(data, 'selectedModel', validated.selectedModel);

  if (selectedProvider !== undefined) {
    trimmed.selectedProvider = selectedProvider;
  }
  if (selectedModel !== undefined) {
    trimmed.selectedModel = selectedModel;
  }

  return patch(apiBaseUrl, '/api/settings/ai-agents', trimmed);
}

export async function testAiAgentsConnection(apiBaseUrl: string, data: unknown) {
  const validated = updateAiAgentsRequestSchema.parse(normalizeAiAgentsPayload(data));
  const body: Record<string, string> = {};

  const openaiApiKey = trimToUndefined(validated.openaiApiKey);
  const geminiApiKey = trimToUndefined(validated.geminiApiKey);
  const openrouterApiKey = trimToUndefined(validated.openrouterApiKey);

  if (openaiApiKey !== undefined) {
    body.openaiApiKey = openaiApiKey;
  }
  if (geminiApiKey !== undefined) {
    body.geminiApiKey = geminiApiKey;
  }
  if (openrouterApiKey !== undefined) {
    body.openrouterApiKey = openrouterApiKey;
  }
  const selectedProvider = getSelectionPatchValue(data, 'selectedProvider', validated.selectedProvider);
  const selectedModel = getSelectionPatchValue(data, 'selectedModel', validated.selectedModel);

  if (selectedProvider !== undefined) {
    body.selectedProvider = selectedProvider;
  }
  if (selectedModel !== undefined) {
    body.selectedModel = selectedModel;
  }

  const parsed = await post<typeof body, unknown>(apiBaseUrl, '/api/settings/ai-agents/test', body, {
    allowedErrorStatuses: [400, 503],
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
    allowedErrorStatuses: [400, 503],
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
