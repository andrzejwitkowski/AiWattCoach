import { afterEach, describe, expect, it, vi } from 'vitest';

import { loadBackendStatus } from './system';

const originalFetch = global.fetch;

afterEach(() => {
  global.fetch = originalFetch;
  vi.restoreAllMocks();
});

describe('loadBackendStatus', () => {
  it('loads health and readiness payloads from the backend API', async () => {
    const fetchMock = vi.fn<(input: RequestInfo | URL, init?: RequestInit) => Promise<Response>>()
      .mockResolvedValueOnce(
        new Response(JSON.stringify({ status: 'ok', service: 'AiWattCoach' }), {
          status: 200,
          headers: { 'content-type': 'application/json' }
        })
      )
      .mockResolvedValueOnce(
        new Response(JSON.stringify({ status: 'ok', reason: null }), {
          status: 200,
          headers: { 'content-type': 'application/json' }
        })
      );

    global.fetch = fetchMock as typeof fetch;

    const result = await loadBackendStatus('');

    expect(fetchMock).toHaveBeenNthCalledWith(1, '/health', expect.any(Object));
    expect(fetchMock).toHaveBeenNthCalledWith(2, '/ready', expect.any(Object));
    expect(result.state).toBe('online');
    expect(result.health.service).toBe('AiWattCoach');
    expect(result.readiness.status).toBe('ok');
    expect(result.checkedAtLabel).not.toBe('just now');
  });

  it('keeps degraded readiness as degraded instead of offline', async () => {
    const fetchMock = vi.fn<(input: RequestInfo | URL, init?: RequestInit) => Promise<Response>>()
      .mockResolvedValueOnce(
        new Response(JSON.stringify({ status: 'ok', service: 'AiWattCoach' }), {
          status: 200,
          headers: { 'content-type': 'application/json' }
        })
      )
      .mockResolvedValueOnce(
        new Response(JSON.stringify({ status: 'degraded', reason: 'mongo_unreachable' }), {
          status: 503,
          headers: { 'content-type': 'application/json' }
        })
      );

    global.fetch = fetchMock as typeof fetch;

    const result = await loadBackendStatus('http://api.example.com');

    expect(fetchMock).toHaveBeenNthCalledWith(
      1,
      'http://api.example.com/health',
      expect.any(Object)
    );
    expect(fetchMock).toHaveBeenNthCalledWith(
      2,
      'http://api.example.com/ready',
      expect.any(Object)
    );
    expect(result.state).toBe('degraded');
    expect(result.readiness.status).toBe('degraded');
    expect(result.readiness.reason).toBe('mongo_unreachable');
  });
});
