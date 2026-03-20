export function normalizeApiBaseUrl(rawValue?: string | null): string {
  if (typeof rawValue !== 'string') {
    return '';
  }

  const normalized = rawValue.trim().replace(/\/+$/, '');

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
  return normalizeApiBaseUrl(import.meta.env.VITE_API_BASE_URL);
}
