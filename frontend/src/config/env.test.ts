import { afterEach, describe, expect, it, vi } from 'vitest';

import { getApiBaseUrl, normalizeApiBaseUrl } from './env';

afterEach(() => {
  vi.unstubAllEnvs();
  vi.restoreAllMocks();
});

describe('normalizeApiBaseUrl', () => {
  it('defaults to same-origin when no override is provided', () => {
    expect(normalizeApiBaseUrl()).toBe('');
    expect(normalizeApiBaseUrl('   ')).toBe('');
  });

  it('trims surrounding whitespace and trailing slashes', () => {
    expect(normalizeApiBaseUrl(' http://127.0.0.1:3002/ ')).toBe('http://127.0.0.1:3002');
  });

  it('accepts root-relative paths and rejects ambiguous relative values', () => {
    expect(normalizeApiBaseUrl('/')).toBe('');
    expect(normalizeApiBaseUrl('/api/')).toBe('/api');
    expect(() => normalizeApiBaseUrl('api')).toThrow(
      'VITE_API_BASE_URL must be empty, an absolute http(s) URL, or a root-relative path'
    );
  });

  it('does not produce protocol-relative URLs when using root as base', () => {
    vi.stubEnv('VITE_API_BASE_URL', '/');

    const baseUrl = getApiBaseUrl();
    expect(baseUrl).toBe('');

    const path = '/health';
    const fullUrl = `${baseUrl}${path}`;
    expect(fullUrl.startsWith('//')).toBe(false);
  });
  it('rejects protocol-relative values', () => {
    expect(() => normalizeApiBaseUrl('//evil.example')).toThrow(
      'VITE_API_BASE_URL must be empty, an absolute http(s) URL, or a root-relative path'
    );
    expect(() => normalizeApiBaseUrl('//')).toThrow(
      'VITE_API_BASE_URL must be empty, an absolute http(s) URL, or a root-relative path'
    );
    expect(() => normalizeApiBaseUrl(' /// ')).toThrow(
      'VITE_API_BASE_URL must be empty, an absolute http(s) URL, or a root-relative path'
    );
  });

  it('falls back to same-origin when the environment value is invalid', () => {
    const warnSpy = vi.spyOn(console, 'warn').mockImplementation(() => {});
    vi.stubEnv('VITE_API_BASE_URL', 'api');

    expect(getApiBaseUrl()).toBe('');
    expect(warnSpy).toHaveBeenCalledTimes(1);
  });
});
