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
});
