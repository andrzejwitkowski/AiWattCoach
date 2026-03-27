export function normalizeApiBaseUrl(rawValue?: string | null): string {
  if (typeof rawValue !== 'string') {
    return '';
  }

  const trimmed = rawValue.trim();

  if (/^\/{2,}/.test(trimmed)) {
    throw new Error(
      'VITE_API_BASE_URL must be empty, an absolute http(s) URL, or a root-relative path'
    );
  }

  const normalized = trimmed === '/' ? '' : trimmed.replace(/\/+$/, '');

  if (!normalized) {
    return '';
  }

  if (normalized.startsWith('/')) {
    return normalized;
  }

  if (/^https?:\/\//.test(normalized)) {
    return normalized;
  }

  throw new Error(
    'VITE_API_BASE_URL must be empty, an absolute http(s) URL, or a root-relative path'
  );
}

export function getApiBaseUrl(): string {
  try {
    return normalizeApiBaseUrl(import.meta.env.VITE_API_BASE_URL);
  } catch (error) {
    console.warn(
      'Invalid VITE_API_BASE_URL. Falling back to same-origin requests.',
      error
    );

    return '';
  }
}

export function normalizeBooleanFlag(rawValue?: string | null): boolean {
  if (typeof rawValue !== 'string') {
    return false;
  }

  const normalized = rawValue.trim().toLowerCase();

  if (!normalized) {
    return false;
  }

  if (normalized === 'true') {
    return true;
  }

  if (normalized === 'false') {
    return false;
  }

  throw new Error('Expected a boolean environment flag value of true or false');
}

export function isDevAuthEnabled(): boolean {
  try {
    return normalizeBooleanFlag(import.meta.env.VITE_DEV_AUTH_ENABLED);
  } catch (error) {
    console.warn(
      'Invalid VITE_DEV_AUTH_ENABLED value. Falling back to disabled dev auth hint.',
      error
    );

    return false;
  }
}
