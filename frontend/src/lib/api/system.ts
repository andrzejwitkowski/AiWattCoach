import { ApiError, getJsonResponse } from './client';

export type HealthPayload = {
  status: string;
  service: string;
};

export type ReadinessPayload = {
  status: string;
  reason: string | null;
};

export type BackendStatusState = 'online' | 'degraded' | 'offline';

export type BackendStatusKind = BackendStatusState | 'loading';

export type BackendStatus = {
  health: HealthPayload;
  readiness: ReadinessPayload;
  state: BackendStatusKind;
  checkedAtLabel: string;
};

function buildApiUrl(apiBaseUrl: string, path: string): string {
  const normalizedPath = path.startsWith('/') ? path : `/${path}`;

  if (!apiBaseUrl) {
    return normalizedPath;
  }

  return `${apiBaseUrl}${normalizedPath}`;
}

function formatCheckedAtLabel(date: Date): string {
  return date.toLocaleTimeString([], {
    hour: '2-digit',
    minute: '2-digit',
    second: '2-digit'
  });
}

export async function loadBackendStatus(apiBaseUrl: string): Promise<BackendStatus> {
  const [healthResponse, readinessResponse] = await Promise.all([
    getJsonResponse<HealthPayload>(buildApiUrl(apiBaseUrl, '/health')),
    getJsonResponse<ReadinessPayload>(buildApiUrl(apiBaseUrl, '/ready'))
  ]);

  if (!healthResponse.ok) {
    throw new ApiError(`Health endpoint returned unexpected status ${healthResponse.status}`);
  }

  if (!readinessResponse.ok && readinessResponse.status !== 503) {
    throw new ApiError(
      `Readiness endpoint returned unexpected status ${readinessResponse.status}`
    );
  }

  const checkedAtLabel = formatCheckedAtLabel(new Date());

  return {
    health: healthResponse.body,
    readiness: readinessResponse.body,
    state: readinessResponse.status === 503 ? 'degraded' : 'online',
    checkedAtLabel
  };
}
