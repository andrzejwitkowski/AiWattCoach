import { afterEach, describe, expect, it, vi } from 'vitest';

import { testIntervalsConnection } from './settings';
import { AuthenticationError, HttpError } from '../../../lib/httpClient';

const originalFetch = global.fetch;

afterEach(() => {
  global.fetch = originalFetch;
  vi.restoreAllMocks();
});

describe('settings api', () => {
  it('posts intervals test credentials and parses a successful response', async () => {
    const fetchMock = vi
      .fn<(input: RequestInfo | URL, init?: RequestInit) => Promise<Response>>()
      .mockResolvedValue(
        new Response(
          JSON.stringify({
            connected: true,
            message: 'Connection successful.',
            usedSavedApiKey: false,
            usedSavedAthleteId: false,
            persistedStatusUpdated: false,
          }),
          {
            status: 200,
            headers: { 'content-type': 'application/json' },
          },
        ),
      );

    global.fetch = fetchMock as typeof fetch;

    const result = await testIntervalsConnection('', {
      apiKey: 'live-api-key',
      athleteId: 'athlete-123',
    });

    expect(fetchMock).toHaveBeenCalledWith('/api/settings/intervals/test', {
      method: 'POST',
      headers: {
        Accept: 'application/json',
        'Content-Type': 'application/json',
        traceparent: expect.stringMatching(/^[0-9a-f]{2}-[0-9a-f]{32}-[0-9a-f]{16}-[0-9a-f]{2}$/),
      },
      credentials: 'include',
      body: JSON.stringify({
        apiKey: 'live-api-key',
        athleteId: 'athlete-123',
      }),
    });
    expect(result.connected).toBe(true);
    expect(result.message).toBe('Connection successful.');
  });

  it('parses handled failure responses from the test endpoint', async () => {
    global.fetch = vi
      .fn<(input: RequestInfo | URL, init?: RequestInit) => Promise<Response>>()
      .mockResolvedValue(
        new Response(
          JSON.stringify({
            connected: false,
            message: 'Invalid API key or athlete ID. Please check your credentials.',
            usedSavedApiKey: true,
            usedSavedAthleteId: false,
            persistedStatusUpdated: false,
          }),
          {
            status: 400,
            headers: { 'content-type': 'application/json' },
          },
        ),
      ) as typeof fetch;

    const result = await testIntervalsConnection('', {
      athleteId: 'athlete-123',
    });

    expect(result.connected).toBe(false);
    expect(result.usedSavedApiKey).toBe(true);
    expect(result.message).toContain('Invalid API key');
  });

  it('throws AuthenticationError for unauthorized test requests', async () => {
    global.fetch = vi
      .fn<(input: RequestInfo | URL, init?: RequestInit) => Promise<Response>>()
      .mockResolvedValue(new Response(null, { status: 401 })) as typeof fetch;

    await expect(testIntervalsConnection('', { athleteId: 'athlete-123' })).rejects.toBeInstanceOf(
      AuthenticationError,
    );
  });

  it('parses handled 503 responses from the test endpoint', async () => {
    global.fetch = vi
      .fn<(input: RequestInfo | URL, init?: RequestInit) => Promise<Response>>()
      .mockResolvedValue(
        new Response(
          JSON.stringify({
            connected: false,
            message: 'Intervals.icu is currently unavailable. Please try again later.',
            usedSavedApiKey: false,
            usedSavedAthleteId: true,
            persistedStatusUpdated: false,
          }),
          {
            status: 503,
            headers: { 'content-type': 'application/json' },
          },
        ),
      ) as typeof fetch;

    const result = await testIntervalsConnection('', {
      apiKey: 'live-api-key',
    });

    expect(result.connected).toBe(false);
    expect(result.usedSavedAthleteId).toBe(true);
    expect(result.message).toContain('unavailable');
  });

  it('throws HttpError for unexpected statuses', async () => {
    global.fetch = vi
      .fn<(input: RequestInfo | URL, init?: RequestInit) => Promise<Response>>()
      .mockResolvedValue(new Response(null, { status: 418 })) as typeof fetch;

    await expect(testIntervalsConnection('', { apiKey: 'live-api-key' })).rejects.toBeInstanceOf(
      HttpError,
    );
  });

  it('throws HttpError for invalid json responses', async () => {
    global.fetch = vi
      .fn<(input: RequestInfo | URL, init?: RequestInit) => Promise<Response>>()
      .mockResolvedValue(
        new Response('not-json', {
          status: 503,
          headers: { 'content-type': 'application/json' },
        }),
      ) as typeof fetch;

    await expect(testIntervalsConnection('', { apiKey: 'live-api-key' })).rejects.toBeInstanceOf(
      HttpError,
    );
  });
});
