import { getJsonResponse } from '../../../lib/api/client';
import { post } from '../../../lib/httpClient';
import type { CurrentUserResponse, JoinWhitelistResponse } from '../types';

function buildAuthUrl(apiBaseUrl: string, path: string): string {
  if (!apiBaseUrl) {
    return path;
  }

  return `${apiBaseUrl}${path}`;
}

export async function loadCurrentUser(apiBaseUrl: string): Promise<CurrentUserResponse> {
  const response = await getJsonResponse<CurrentUserResponse>(buildAuthUrl(apiBaseUrl, '/api/auth/me'), {
    credentials: 'include'
  });

  return response.body;
}

export function buildGoogleLoginUrl(apiBaseUrl: string, returnTo = '/calendar'): string {
  const params = new URLSearchParams({ returnTo });
  return `${buildAuthUrl(apiBaseUrl, '/api/auth/google/start')}?${params.toString()}`;
}

export async function logout(apiBaseUrl: string): Promise<void> {
  await fetch(buildAuthUrl(apiBaseUrl, '/api/auth/logout'), {
    method: 'POST',
    credentials: 'include'
  });
}

export async function joinWhitelist(apiBaseUrl: string, email: string): Promise<JoinWhitelistResponse> {
  return post<{ email: string }, JoinWhitelistResponse>(apiBaseUrl, '/api/auth/whitelist', {
    email
  });
}

export type AdminSystemInfo = {
  appName: string;
  mongoDatabase: string;
};

export async function loadAdminSystemInfo(apiBaseUrl: string): Promise<AdminSystemInfo> {
  const response = await getJsonResponse<AdminSystemInfo>(
    buildAuthUrl(apiBaseUrl, '/api/admin/system-info'),
    {
      credentials: 'include'
    }
  );

  return response.body;
}
