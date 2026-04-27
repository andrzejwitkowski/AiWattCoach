import { describe, expect, it, vi, afterEach } from 'vitest';

import { post, HttpError } from './httpClient';

afterEach(() => {
  vi.restoreAllMocks();
});

describe('httpClient', () => {
  it('uses validation message bodies for HttpError messages', async () => {
    vi.stubGlobal('fetch', vi.fn().mockResolvedValue({
      status: 400,
      ok: false,
      json: vi.fn().mockResolvedValue({ message: 'specific validation failure' }),
    }));

    await expect(post('', '/api/test', undefined)).rejects.toMatchObject({
      status: 400,
      message: 'specific validation failure',
    } satisfies Partial<HttpError>);
  });

  it('falls back to generic message when error body has no message', async () => {
    vi.stubGlobal('fetch', vi.fn().mockResolvedValue({
      status: 400,
      ok: false,
      json: vi.fn().mockResolvedValue({ foo: 'bar' }),
    }));

    await expect(post('', '/api/test', undefined)).rejects.toMatchObject({
      status: 400,
      message: 'POST /api/test failed: 400',
    } satisfies Partial<HttpError>);
  });

  it('falls back to generic message when message is empty or non-string', async () => {
    vi.stubGlobal('fetch', vi.fn().mockResolvedValue({
      status: 422,
      ok: false,
      json: vi.fn().mockResolvedValue({ message: '   ' }),
    }));

    await expect(post('', '/api/test', undefined)).rejects.toMatchObject({
      status: 422,
      message: 'POST /api/test failed: 422',
    } satisfies Partial<HttpError>);
  });

  it('throws invalid JSON response on successful malformed JSON body', async () => {
    vi.stubGlobal('fetch', vi.fn().mockResolvedValue({
      status: 200,
      ok: true,
      json: vi.fn().mockRejectedValue(new Error('bad json')),
    }));

    await expect(post('', '/api/test', undefined)).rejects.toMatchObject({
      status: 200,
      message: 'POST /api/test: invalid JSON response',
    } satisfies Partial<HttpError>);
  });
});
