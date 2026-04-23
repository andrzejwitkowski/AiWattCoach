import { act, cleanup, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { CompletedWorkoutsProvider, useCompletedWorkouts } from './CompletedWorkoutsContext';

const originalFetch = global.fetch;

afterEach(() => {
  global.fetch = originalFetch;
  vi.restoreAllMocks();
  cleanup();
});

function makeActivityResponse() {
  return [
    {
      id: 'i1',
      startDateLocal: '2026-03-15T08:00:00',
      startDate: null,
      name: 'Morning Ride',
      description: null,
      activityType: 'Ride',
      source: 'canonical_completed_workout',
      externalId: null,
      deviceName: null,
      distanceMeters: 40000,
      movingTimeSeconds: 3600,
      elapsedTimeSeconds: 3700,
      totalElevationGainMeters: 420,
      averageSpeedMps: 11.1,
      averageHeartRateBpm: 148,
      averageCadenceRpm: 88.5,
      trainer: false,
      commute: false,
      race: false,
      hasHeartRate: true,
      streamTypes: ['watts'],
      tags: [],
      metrics: {
        trainingStressScore: 74,
        normalizedPowerWatts: 238,
        intensityFactor: 0.84,
        efficiencyFactor: 1.29,
        variabilityIndex: 1.05,
        averagePowerWatts: 227,
        ftpWatts: 283,
        totalWorkJoules: 820,
        calories: 700,
        trimp: 90,
        powerLoad: 74,
        heartRateLoad: 68,
        paceLoad: null,
        strainScore: 13.5,
      },
      details: {
        intervals: [],
        intervalGroups: [],
        streams: [],
        intervalSummary: [],
        skylineChart: [],
        powerZoneTimes: [],
        heartRateZoneTimes: [],
        paceZoneTimes: [],
        gapZoneTimes: [],
      },
      detailsUnavailableReason: null,
    },
  ];
}

function renderProvider() {
  const fetchMock = vi.fn<(input: RequestInfo | URL, init?: RequestInit) => Promise<Response>>();
  global.fetch = fetchMock as typeof fetch;

  function Probe() {
    const ctx = useCompletedWorkouts();
    return (
      <div>
        <span data-testid="loading">{String(ctx.isLoading)}</span>
        <span data-testid="error">{ctx.error ? `${ctx.error.kind}${ctx.error.kind === 'network-error' ? `: ${ctx.error.message}` : ''}` : 'none'}</span>
        <button data-testid="fetch" onClick={async () => {
          try {
            await ctx.getActivitiesForRange('2026-03-01', '2026-03-31');
          } catch {
            // ignored in tests
          }
        }}>fetch</button>
        <button data-testid="invalidate-range" onClick={() => ctx.invalidateRange('2026-03-01', '2026-03-31')}>invalidate range</button>
        <button data-testid="invalidate-all" onClick={() => ctx.invalidateAll()}>invalidate all</button>
      </div>
    );
  }

  return {
    fetchMock,
    renderResult: render(
      <CompletedWorkoutsProvider apiBaseUrl="">
        <Probe />
      </CompletedWorkoutsProvider>,
    ),
  };
}

describe('CompletedWorkoutsProvider', () => {
  it('fetches activities on first call', async () => {
    const { fetchMock } = renderProvider();

    await act(async () => {
      screen.getByTestId('fetch').click();
    });

    await waitFor(() => {
      expect(fetchMock).toHaveBeenCalledTimes(1);
    });

    expect(fetchMock).toHaveBeenCalledWith(
      '/api/completed-workouts?oldest=2026-03-01&newest=2026-03-31',
      expect.objectContaining({ method: 'GET', credentials: 'include' }),
    );
  });

  it('returns cached data without re-fetching for the same range', async () => {
    const { fetchMock } = renderProvider();

    fetchMock.mockResolvedValue(
      new Response(
        JSON.stringify(makeActivityResponse()),
        { status: 200, headers: { 'content-type': 'application/json' } },
      ),
    );

    await act(async () => {
      screen.getByTestId('fetch').click();
    });

    await waitFor(() => {
      expect(fetchMock).toHaveBeenCalledTimes(1);
    });

    await act(async () => {
      screen.getByTestId('fetch').click();
    });

    await new Promise((resolve) => setTimeout(resolve, 50));

    expect(fetchMock).toHaveBeenCalledTimes(1);
  });

  it('deduplicates concurrent requests for the same range', async () => {
    const { fetchMock } = renderProvider();

    let resolveFetch: (response: Response) => void;
    const fetchPromise = new Promise<Response>((resolve) => {
      resolveFetch = resolve;
    });
    fetchMock.mockReturnValue(fetchPromise);

    await act(async () => {
      screen.getByTestId('fetch').click();
      screen.getByTestId('fetch').click();
      screen.getByTestId('fetch').click();
    });

    await act(async () => {
      resolveFetch!(
        new Response(
          JSON.stringify(makeActivityResponse()),
          { status: 200, headers: { 'content-type': 'application/json' } },
        ),
      );
    });

    await waitFor(() => {
      expect(fetchMock).toHaveBeenCalledTimes(1);
    });
  });

  it('invalidates specific range cache', async () => {
    const { fetchMock } = renderProvider();

    fetchMock.mockResolvedValue(
      new Response(
        JSON.stringify(makeActivityResponse()),
        { status: 200, headers: { 'content-type': 'application/json' } },
      ),
    );

    await act(async () => {
      screen.getByTestId('fetch').click();
    });

    await waitFor(() => {
      expect(fetchMock).toHaveBeenCalledTimes(1);
    });

    await act(async () => {
      screen.getByTestId('invalidate-range').click();
    });

    await act(async () => {
      screen.getByTestId('fetch').click();
    });

    await waitFor(() => {
      expect(fetchMock).toHaveBeenCalledTimes(2);
    });
  });

  it('invalidates all cache entries', async () => {
    const { fetchMock } = renderProvider();

    fetchMock.mockResolvedValue(
      new Response(
        JSON.stringify(makeActivityResponse()),
        { status: 200, headers: { 'content-type': 'application/json' } },
      ),
    );

    await act(async () => {
      screen.getByTestId('fetch').click();
    });

    await waitFor(() => {
      expect(fetchMock).toHaveBeenCalledTimes(1);
    });

    await act(async () => {
      screen.getByTestId('invalidate-all').click();
    });

    await act(async () => {
      screen.getByTestId('fetch').click();
    });

    await waitFor(() => {
      expect(fetchMock).toHaveBeenCalledTimes(2);
    });
  });

  it('shows loading state during fetch', async () => {
    const { fetchMock } = renderProvider();

    let resolveFetch: (response: Response) => void;
    const fetchPromise = new Promise<Response>((resolve) => {
      resolveFetch = resolve;
    });
    fetchMock.mockReturnValue(fetchPromise);

    await act(async () => {
      screen.getByTestId('fetch').click();
    });

    expect(screen.getByTestId('loading').textContent).toBe('true');

    await act(async () => {
      resolveFetch!(
        new Response(
          JSON.stringify(makeActivityResponse()),
          { status: 200, headers: { 'content-type': 'application/json' } },
        ),
      );
    });

    await waitFor(() => {
      expect(screen.getByTestId('loading').textContent).toBe('false');
    });
  });

  it('surfaces error on non-auth failure', async () => {
    const { fetchMock } = renderProvider();

    fetchMock.mockResolvedValue(
      new Response(null, { status: 500 }),
    );

    await act(async () => {
      screen.getByTestId('fetch').click();
    });

    await waitFor(() => {
      const errorText = screen.getByTestId('error').textContent;
      expect(errorText).toContain('network-error');
    });
  });

  it('clears error on successful cache hit', async () => {
    const { fetchMock } = renderProvider();

    fetchMock.mockResolvedValueOnce(
      new Response(null, { status: 500 }),
    );

    await act(async () => {
      screen.getByTestId('fetch').click();
    });

    await waitFor(() => {
      const errorText = screen.getByTestId('error').textContent;
      expect(errorText).toContain('network-error');
    });

    fetchMock.mockResolvedValueOnce(
      new Response(
        JSON.stringify(makeActivityResponse()),
        { status: 200, headers: { 'content-type': 'application/json' } },
      ),
    );

    await act(async () => {
      screen.getByTestId('fetch').click();
    });

    await waitFor(() => {
      expect(screen.getByTestId('error').textContent).toBe('none');
    });
  });

  it('surfaces credentials-required error on 422', async () => {
    const { fetchMock } = renderProvider();

    fetchMock.mockResolvedValue(
      new Response(null, { status: 422 }),
    );

    await act(async () => {
      screen.getByTestId('fetch').click();
    });

    await waitFor(() => {
      expect(screen.getByTestId('error').textContent).toContain('credentials-required');
    });
  });
});
