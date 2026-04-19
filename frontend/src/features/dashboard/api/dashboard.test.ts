import { describe, expect, it } from 'vitest';

import { createFetchMock, useFetchMock } from '../../intervals/api/testHelpers';
import { loadTrainingLoadDashboard } from './dashboard';

describe('dashboard api', () => {
  it('loads training load report for selected range', async () => {
    const fetchMock = useFetchMock(
      createFetchMock().mockResolvedValueOnce(
        new Response(
          JSON.stringify({
            range: '90d',
            windowStart: '2026-01-19',
            windowEnd: '2026-04-18',
            hasTrainingLoad: true,
            summary: {
              currentCtl: 29.9,
              currentAtl: 53.4,
              currentTsb: -23.5,
              ftpWatts: 340,
              averageIf28d: 72.95,
              averageEf28d: null,
              loadDeltaCtl14d: 8.4,
              tsbZone: 'optimal_training',
            },
            points: [
              {
                date: '2026-04-18',
                dailyTss: 97,
                currentCtl: 29.9,
                currentAtl: 53.4,
                currentTsb: -23.5,
              },
            ],
          }),
          { status: 200, headers: { 'content-type': 'application/json' } },
        ),
      ),
    );

    const result = await loadTrainingLoadDashboard('', '90d');

    expect(fetchMock).toHaveBeenCalledWith('/api/dashboard/training-load?range=90d', {
      method: 'GET',
      headers: {
        Accept: 'application/json',
        traceparent: expect.stringMatching(/^[0-9a-f]{2}-[0-9a-f]{32}-[0-9a-f]{16}-[0-9a-f]{2}$/),
      },
      credentials: 'include',
      body: undefined,
    });
    expect(result.summary.currentCtl).toBe(29.9);
    expect(result.points).toHaveLength(1);
  });
});
