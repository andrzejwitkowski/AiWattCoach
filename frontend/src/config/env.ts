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

  const normalized = trimmed === '/' ? trimmed : trimmed.replace(/\/+$/, '');

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
