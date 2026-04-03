import { afterEach, describe, expect, it, vi } from 'vitest';

import { testAiAgentsConnection, testIntervalsConnection, updateAiAgents, updateCycling, updateIntervals } from './settings';
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
    expect(result).toEqual({
      connected: true,
      message: 'Connection successful.',
      usedSavedApiKey: false,
      usedSavedAthleteId: false,
      persistedStatusUpdated: false,
    });
  });

  it('throws AuthenticationError for 401 settings responses', async () => {
    global.fetch = vi
      .fn<(input: RequestInfo | URL, init?: RequestInit) => Promise<Response>>()
      .mockResolvedValue(
        new Response(JSON.stringify({ message: 'Unauthorized' }), {
          status: 401,
          headers: { 'content-type': 'application/json' },
        }),
      ) as typeof fetch;

    await expect(testIntervalsConnection('', { apiKey: 'live-api-key' })).rejects.toBeInstanceOf(
      AuthenticationError,
    );
  });

  it('throws HttpError for unhandled settings responses', async () => {
    global.fetch = vi
      .fn<(input: RequestInfo | URL, init?: RequestInit) => Promise<Response>>()
      .mockResolvedValue(
        new Response(JSON.stringify({ message: 'Server error' }), {
          status: 500,
          headers: { 'content-type': 'application/json' },
        }),
      ) as typeof fetch;

    await expect(testIntervalsConnection('', { apiKey: 'live-api-key' })).rejects.toBeInstanceOf(
      HttpError,
    );
  });

  it('preserves explicit clears in intervals update requests', async () => {
    const fetchMock = vi
      .fn<(input: RequestInfo | URL, init?: RequestInit) => Promise<Response>>()
      .mockResolvedValue(
        new Response('{}', {
          status: 200,
          headers: { 'content-type': 'application/json' },
        }),
      );

    global.fetch = fetchMock as typeof fetch;

    await updateIntervals('', {
      apiKey: '',
      athleteId: null,
    });

    expect(fetchMock).toHaveBeenCalledWith('/api/settings/intervals', {
      method: 'PATCH',
      headers: {
        Accept: 'application/json',
        'Content-Type': 'application/json',
        traceparent: expect.stringMatching(/^[0-9a-f]{2}-[0-9a-f]{32}-[0-9a-f]{16}-[0-9a-f]{2}$/),
      },
      credentials: 'include',
      body: JSON.stringify({
        apiKey: null,
        athleteId: null,
      }),
    });
  });

  it('posts ai test settings and parses a successful response', async () => {
    const fetchMock = vi
      .fn<(input: RequestInfo | URL, init?: RequestInit) => Promise<Response>>()
      .mockResolvedValue(
        new Response(
          JSON.stringify({
            connected: true,
            message: 'Connection successful.',
            usedSavedApiKey: false,
            usedSavedProvider: false,
            usedSavedModel: false,
          }),
          {
            status: 200,
            headers: { 'content-type': 'application/json' },
          },
        ),
      );

    global.fetch = fetchMock as typeof fetch;

    const result = await testAiAgentsConnection('', {
      openrouterApiKey: 'or-key',
      selectedProvider: 'openrouter',
      selectedModel: 'openai/gpt-4o-mini',
    });

    expect(fetchMock).toHaveBeenCalledWith('/api/settings/ai-agents/test', {
      method: 'POST',
      headers: {
        Accept: 'application/json',
        'Content-Type': 'application/json',
        traceparent: expect.stringMatching(/^[0-9a-f]{2}-[0-9a-f]{32}-[0-9a-f]{16}-[0-9a-f]{2}$/),
      },
      credentials: 'include',
      body: JSON.stringify({
        openrouterApiKey: 'or-key',
        selectedProvider: 'openrouter',
        selectedModel: 'openai/gpt-4o-mini',
      }),
    });
    expect(result).toEqual({
      connected: true,
      message: 'Connection successful.',
      usedSavedApiKey: false,
      usedSavedProvider: false,
      usedSavedModel: false,
    });
  });

  it('parses handled ai connection failure responses', async () => {
    global.fetch = vi
      .fn<(input: RequestInfo | URL, init?: RequestInit) => Promise<Response>>()
      .mockResolvedValue(
        new Response(
          JSON.stringify({
            connected: false,
            message: 'Provider, model, and matching API key are required.',
            usedSavedApiKey: true,
            usedSavedProvider: false,
            usedSavedModel: false,
          }),
          {
            status: 400,
            headers: { 'content-type': 'application/json' },
          },
        ),
      ) as typeof fetch;

    const result = await testAiAgentsConnection('', {
      selectedProvider: 'openrouter',
    });

    expect(result).toEqual({
      connected: false,
      message: 'Provider, model, and matching API key are required.',
      usedSavedApiKey: true,
      usedSavedProvider: false,
      usedSavedModel: false,
    });
  });

  it('omits whitespace-only ai settings fields from update requests', async () => {
    const fetchMock = vi
      .fn<(input: RequestInfo | URL, init?: RequestInit) => Promise<Response>>()
      .mockResolvedValue(
        new Response('{}', {
          status: 200,
          headers: { 'content-type': 'application/json' },
        }),
      );

    global.fetch = fetchMock as typeof fetch;

    await updateAiAgents('', {
      openaiApiKey: '   ',
      geminiApiKey: ' gem-key ',
      selectedProvider: ' openai ',
      selectedModel: '   ',
    });

    expect(fetchMock).toHaveBeenCalledWith('/api/settings/ai-agents', {
      method: 'PATCH',
      headers: {
        Accept: 'application/json',
        'Content-Type': 'application/json',
        traceparent: expect.stringMatching(/^[0-9a-f]{2}-[0-9a-f]{32}-[0-9a-f]{16}-[0-9a-f]{2}$/),
      },
      credentials: 'include',
      body: JSON.stringify({
        geminiApiKey: 'gem-key',
        selectedProvider: 'openai',
      }),
    });
  });

  it('preserves explicit provider and model clears in update requests', async () => {
    const fetchMock = vi
      .fn<(input: RequestInfo | URL, init?: RequestInit) => Promise<Response>>()
      .mockResolvedValue(
        new Response('{}', {
          status: 200,
          headers: { 'content-type': 'application/json' },
        }),
      );

    global.fetch = fetchMock as typeof fetch;

    await updateAiAgents('', {
      selectedProvider: '',
      selectedModel: '',
    });

    expect(fetchMock).toHaveBeenCalledWith('/api/settings/ai-agents', {
      method: 'PATCH',
      headers: {
        Accept: 'application/json',
        'Content-Type': 'application/json',
        traceparent: expect.stringMatching(/^[0-9a-f]{2}-[0-9a-f]{32}-[0-9a-f]{16}-[0-9a-f]{2}$/),
      },
      credentials: 'include',
      body: JSON.stringify({
        selectedProvider: null,
        selectedModel: null,
      }),
    });
  });

  it('trims athlete profile context fields in cycling updates', async () => {
    const fetchMock = vi
      .fn<(input: RequestInfo | URL, init?: RequestInit) => Promise<Response>>()
      .mockResolvedValue(
        new Response('{}', {
          status: 200,
          headers: { 'content-type': 'application/json' },
        }),
      );

    global.fetch = fetchMock as typeof fetch;

    await updateCycling('', {
      fullName: ' Alex ',
      athletePrompt: '  Stage-race focus  ',
      medications: '  Iron  ',
      athleteNotes: '  Needs extra recovery  ',
    });

    expect(fetchMock).toHaveBeenCalledWith('/api/settings/cycling', {
      method: 'PATCH',
      headers: {
        Accept: 'application/json',
        'Content-Type': 'application/json',
        traceparent: expect.stringMatching(/^[0-9a-f]{2}-[0-9a-f]{32}-[0-9a-f]{16}-[0-9a-f]{2}$/),
      },
      credentials: 'include',
      body: JSON.stringify({
        fullName: 'Alex',
        athletePrompt: 'Stage-race focus',
        medications: 'Iron',
        athleteNotes: 'Needs extra recovery',
      }),
    });
  });

  it('clears athlete profile context fields when blank values are sent', async () => {
    const fetchMock = vi
      .fn<(input: RequestInfo | URL, init?: RequestInit) => Promise<Response>>()
      .mockResolvedValue(
        new Response('{}', {
          status: 200,
          headers: { 'content-type': 'application/json' },
        }),
      );

    global.fetch = fetchMock as typeof fetch;

    await updateCycling('', {
      athletePrompt: '',
      medications: '   ',
      athleteNotes: null,
    });

    expect(fetchMock).toHaveBeenCalledWith('/api/settings/cycling', {
      method: 'PATCH',
      headers: {
        Accept: 'application/json',
        'Content-Type': 'application/json',
        traceparent: expect.stringMatching(/^[0-9a-f]{2}-[0-9a-f]{32}-[0-9a-f]{16}-[0-9a-f]{2}$/),
      },
      credentials: 'include',
      body: JSON.stringify({
        athletePrompt: null,
        medications: null,
        athleteNotes: null,
      }),
    });
  });

  it('clears full name when a blank value is sent', async () => {
    const fetchMock = vi
      .fn<(input: RequestInfo | URL, init?: RequestInit) => Promise<Response>>()
      .mockResolvedValue(
        new Response('{}', {
          status: 200,
          headers: { 'content-type': 'application/json' },
        }),
      );

    global.fetch = fetchMock as typeof fetch;

    await updateCycling('', {
      fullName: '   ',
    });

    expect(fetchMock).toHaveBeenCalledWith('/api/settings/cycling', {
      method: 'PATCH',
      headers: {
        Accept: 'application/json',
        'Content-Type': 'application/json',
        traceparent: expect.stringMatching(/^[0-9a-f]{2}-[0-9a-f]{32}-[0-9a-f]{16}-[0-9a-f]{2}$/),
      },
      credentials: 'include',
      body: JSON.stringify({
        fullName: null,
      }),
    });
  });
});
