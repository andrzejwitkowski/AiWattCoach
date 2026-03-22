import type {
  UserSettingsResponse,
  UpdateAiAgentsRequest,
  UpdateIntervalsRequest,
  UpdateOptionsRequest,
  UpdateCyclingRequest,
} from '../types';

function buildSettingsUrl(apiBaseUrl: string, path: string): string {
  if (!apiBaseUrl) return path;
  return `${apiBaseUrl}${path}`;
}

async function patchSettings<TReq, TRes>(
  apiBaseUrl: string,
  path: string,
  body: TReq
): Promise<TRes> {
  const response = await fetch(buildSettingsUrl(apiBaseUrl, path), {
    method: 'PATCH',
    headers: { 'Content-Type': 'application/json', Accept: 'application/json' },
    credentials: 'include',
    body: JSON.stringify(body),
  });

  if (!response.ok) {
    throw new Error(`Settings update failed: ${response.status}`);
  }

  return (await response.json()) as TRes;
}

export async function loadSettings(apiBaseUrl: string): Promise<UserSettingsResponse> {
  const response = await fetch(buildSettingsUrl(apiBaseUrl, '/api/settings'), {
    method: 'GET',
    headers: { Accept: 'application/json' },
    credentials: 'include',
  });

  if (!response.ok) {
    if (response.status === 401) {
      throw new Error('401: Unauthorized');
    }
    throw new Error(`Failed to load settings: ${response.status}`);
  }

  return (await response.json()) as UserSettingsResponse;
}

export async function updateAiAgents(
  apiBaseUrl: string,
  data: UpdateAiAgentsRequest
): Promise<UserSettingsResponse> {
  return patchSettings(apiBaseUrl, '/api/settings/ai-agents', data);
}

export async function updateIntervals(
  apiBaseUrl: string,
  data: UpdateIntervalsRequest
): Promise<UserSettingsResponse> {
  return patchSettings(apiBaseUrl, '/api/settings/intervals', data);
}

export async function updateOptions(
  apiBaseUrl: string,
  data: UpdateOptionsRequest
): Promise<UserSettingsResponse> {
  return patchSettings(apiBaseUrl, '/api/settings/options', data);
}

export async function updateCycling(
  apiBaseUrl: string,
  data: UpdateCyclingRequest
): Promise<UserSettingsResponse> {
  return patchSettings(apiBaseUrl, '/api/settings/cycling', data);
}
