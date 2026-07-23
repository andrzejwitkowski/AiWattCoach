import { afterEach, describe, expect, it, vi } from 'vitest';

import { buildTestSettings, defaultAvailabilityDays } from '../mockData';
import { userSettingsResponseSchema } from '../types';
import {
  testAiAgentsConnection,
  testIntervalsConnection,
  updateAiAgents,
  updateAvailability,
  updateCycling,
  updateIntervals,
} from './settings';
import { AuthenticationError, HttpError } from '../../../lib/httpClient';

const originalFetch = global.fetch;

function mockSettingsResponse(settings = buildTestSettings()) {
  return new Response(JSON.stringify(settings), {
    status: 200,
    headers: { 'content-type': 'application/json' },
  });
}

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

  it('patches explicit weekly availability payloads', async () => {
    const responseBody = buildTestSettings({
      aiAgents: {
        openaiApiKey: null,
        openaiApiKeySet: false,
        geminiApiKey: null,
        geminiApiKeySet: false,
        openrouterApiKey: null,
        openrouterApiKeySet: false,
        deepseekApiKey: null,
        deepseekApiKeySet: false,
        zaiApiKey: null,
        zaiApiKeySet: false,
        selectedProvider: null,
        selectedModel: null,
      },
    });
    const fetchMock = vi
      .fn<(input: RequestInfo | URL, init?: RequestInit) => Promise<Response>>()
      .mockResolvedValue(
        new Response(JSON.stringify(responseBody), {
          status: 200,
          headers: { 'content-type': 'application/json' },
        }),
      );

    global.fetch = fetchMock as typeof fetch;

    const result = await updateAvailability('', {
      days: defaultAvailabilityDays,
    });

    expect(fetchMock).toHaveBeenCalledWith('/api/settings/availability', {
      method: 'PATCH',
      headers: {
        Accept: 'application/json',
        'Content-Type': 'application/json',
        traceparent: expect.stringMatching(/^[0-9a-f]{2}-[0-9a-f]{32}-[0-9a-f]{16}-[0-9a-f]{2}$/),
      },
      credentials: 'include',
      body: JSON.stringify({
        days: [
          { weekday: 'mon', available: true, maxDurationMinutes: 60 },
          { weekday: 'tue', available: false, maxDurationMinutes: null },
          { weekday: 'wed', available: true, maxDurationMinutes: 90 },
          { weekday: 'thu', available: false, maxDurationMinutes: null },
          { weekday: 'fri', available: true, maxDurationMinutes: 120 },
          { weekday: 'sat', available: false, maxDurationMinutes: null },
          { weekday: 'sun', available: false, maxDurationMinutes: null },
        ],
      }),
    });

    expect(result).toEqual(responseBody);
  });

  it('surfaces backend validation messages for availability updates', async () => {
    global.fetch = vi
      .fn<(input: RequestInfo | URL, init?: RequestInit) => Promise<Response>>()
      .mockResolvedValue(
        new Response(JSON.stringify({ message: 'availability must contain exactly 7 days' }), {
          status: 400,
          headers: { 'content-type': 'application/json' },
        }),
      ) as typeof fetch;

    await expect(
      updateAvailability('', {
        days: [
          { weekday: 'mon', available: true, maxDurationMinutes: 60 },
          { weekday: 'tue', available: false, maxDurationMinutes: null },
          { weekday: 'wed', available: true, maxDurationMinutes: 90 },
          { weekday: 'thu', available: false, maxDurationMinutes: null },
          { weekday: 'fri', available: true, maxDurationMinutes: 120 },
          { weekday: 'sat', available: false, maxDurationMinutes: null },
          { weekday: 'sun', available: false, maxDurationMinutes: null },
        ],
      }),
    ).rejects.toThrow('Failed to update availability: availability must contain exactly 7 days');
  });

  it('rejects duplicate weekdays in availability payloads before sending', async () => {
    const fetchMock = vi
      .fn<(input: RequestInfo | URL, init?: RequestInit) => Promise<Response>>();
    global.fetch = fetchMock as typeof fetch;

    await expect(updateAvailability('', {
      days: [
        { weekday: 'mon', available: true, maxDurationMinutes: 60 },
        { weekday: 'mon', available: false, maxDurationMinutes: null },
        { weekday: 'wed', available: true, maxDurationMinutes: 90 },
        { weekday: 'thu', available: false, maxDurationMinutes: null },
        { weekday: 'fri', available: true, maxDurationMinutes: 120 },
        { weekday: 'sat', available: false, maxDurationMinutes: null },
        { weekday: 'sun', available: false, maxDurationMinutes: null },
      ],
    })).rejects.toThrow(/each weekday exactly once/i);

    expect(fetchMock).not.toHaveBeenCalled();
  });

  it('rejects duplicate weekdays in settings responses', () => {
    expect(() =>
      userSettingsResponseSchema.parse(
        buildTestSettings({
          availability: {
            configured: true,
            days: [
              { weekday: 'mon', available: true, maxDurationMinutes: 60 },
              { weekday: 'mon', available: false, maxDurationMinutes: null },
              { weekday: 'wed', available: true, maxDurationMinutes: 90 },
              { weekday: 'thu', available: false, maxDurationMinutes: null },
              { weekday: 'fri', available: true, maxDurationMinutes: 120 },
              { weekday: 'sat', available: false, maxDurationMinutes: null },
              { weekday: 'sun', available: false, maxDurationMinutes: null },
            ],
          },
        }),
      ),
    ).toThrow(/each weekday exactly once/i);
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

  it('posts deepseek test credentials and parses a successful response', async () => {
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
      deepseekApiKey: 'sk-ds-key',
      selectedProvider: 'deepseek',
      selectedModel: 'deepseek-v4-flash',
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
        deepseekApiKey: 'sk-ds-key',
        selectedProvider: 'deepseek',
        selectedModel: 'deepseek-v4-flash',
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

  it('posts z.ai test credentials and parses a successful response', async () => {
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
      zaiApiKey: 'sk-zai-key',
      selectedProvider: 'zai',
      selectedModel: 'glm-5.2',
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
        zaiApiKey: 'sk-zai-key',
        selectedProvider: 'zai',
        selectedModel: 'glm-5.2',
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
      .mockResolvedValue(mockSettingsResponse());

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

  it('includes deepseek api key in update requests', async () => {
    const fetchMock = vi
      .fn<(input: RequestInfo | URL, init?: RequestInit) => Promise<Response>>()
      .mockResolvedValue(mockSettingsResponse());

    global.fetch = fetchMock as typeof fetch;

    await updateAiAgents('', {
      deepseekApiKey: ' sk-ds-key ',
      selectedProvider: 'deepseek',
      selectedModel: 'deepseek-v4-flash',
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
        deepseekApiKey: 'sk-ds-key',
        selectedProvider: 'deepseek',
        selectedModel: 'deepseek-v4-flash',
      }),
    });
  });

  it('includes openai compatible key and base url in update requests', async () => {
    const fetchMock = vi
      .fn<(input: RequestInfo | URL, init?: RequestInit) => Promise<Response>>()
      .mockResolvedValue(mockSettingsResponse());

    global.fetch = fetchMock as typeof fetch;

    await updateAiAgents('', {
      openaiCompatibleApiKey: ' sk-compat-key ',
      openaiCompatibleBaseUrl: ' http://127.0.0.1:11434/v1/ ',
      selectedProvider: 'openai_compatible',
      selectedModel: 'llama3.2',
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
        openaiCompatibleApiKey: 'sk-compat-key',
        openaiCompatibleBaseUrl: 'http://127.0.0.1:11434/v1/',
        selectedProvider: 'openai_compatible',
        selectedModel: 'llama3.2',
      }),
    });
  });

  it('preserves explicit provider and model clears in update requests', async () => {
    const fetchMock = vi
      .fn<(input: RequestInfo | URL, init?: RequestInit) => Promise<Response>>()
      .mockResolvedValue(mockSettingsResponse());

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

  it('includes post-workout and planning overrides in update requests', async () => {
    const fetchMock = vi
      .fn<(input: RequestInfo | URL, init?: RequestInit) => Promise<Response>>()
      .mockResolvedValue(mockSettingsResponse());

    global.fetch = fetchMock as typeof fetch;

    const result = await updateAiAgents('', {
      workoutChatProvider: 'openai',
      workoutChatModel: 'gpt-5',
      workoutPlanningProvider: 'gemini',
      workoutPlanningModel: 'gemini-2.5-flash',
      mesoCycleProvider: '',
      mesoCycleModel: '',
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
        workoutChatProvider: 'openai',
        workoutChatModel: 'gpt-5',
        workoutPlanningProvider: 'gemini',
        workoutPlanningModel: 'gemini-2.5-flash',
        mesoCycleProvider: null,
        mesoCycleModel: null,
      }),
    });
    expect(result).toEqual(buildTestSettings());
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
