import { describe, expect, it } from 'vitest';

import { normalizeApiBaseUrl } from './env';

describe('normalizeApiBaseUrl', () => {
  it('defaults to same-origin when no override is provided', () => {
    expect(normalizeApiBaseUrl()).toBe('');
    expect(normalizeApiBaseUrl('   ')).toBe('');
  });

  it('trims surrounding whitespace and trailing slashes', () => {
    expect(normalizeApiBaseUrl(' http://127.0.0.1:3002/ ')).toBe('http://127.0.0.1:3002');
  });

  it('accepts root-relative paths and rejects ambiguous relative values', () => {
    expect(normalizeApiBaseUrl('/api/')).toBe('/api');
    expect(() => normalizeApiBaseUrl('api')).toThrow(
      'VITE_API_BASE_URL must be empty, an absolute http(s) URL, or a root-relative path'
    );
  });
});
