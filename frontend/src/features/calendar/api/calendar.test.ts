import { describe, expect, it } from 'vitest';

import { listCalendarLabels } from './calendar';
import { createFetchMock, useFetchMock } from '../../intervals/api/testHelpers';

describe('calendar api', () => {
  it('loads race labels grouped by date', async () => {
    const fetchMock = useFetchMock(
      createFetchMock().mockResolvedValueOnce(
        new Response(
          JSON.stringify({
            labelsByDate: {
              '2026-09-12': {
                'race:race-1': {
                  kind: 'race',
                  title: 'Race Gravel Attack',
                  subtitle: '120 km • Kat. A',
                  payload: {
                    raceId: 'race-1',
                    date: '2026-09-12',
                    name: 'Gravel Attack',
                    distanceMeters: 120000,
                    discipline: 'gravel',
                    priority: 'A',
                    syncStatus: 'synced',
                    linkedIntervalsEventId: 41,
                  },
                },
              },
            },
          }),
          { status: 200, headers: { 'content-type': 'application/json' } },
        ),
      ),
    );

    const result = await listCalendarLabels('', { oldest: '2026-09-01', newest: '2026-09-30' });

    expect(fetchMock).toHaveBeenCalledWith('/api/calendar/labels?oldest=2026-09-01&newest=2026-09-30', {
      method: 'GET',
      headers: {
        Accept: 'application/json',
        traceparent: expect.stringMatching(/^[0-9a-f]{2}-[0-9a-f]{32}-[0-9a-f]{16}-[0-9a-f]{2}$/),
      },
      credentials: 'include',
      body: undefined,
    });
    const raceLabel = result.labelsByDate['2026-09-12']?.['race:race-1'];

    expect(raceLabel?.kind).toBe('race');
    if (raceLabel?.kind === 'race') {
      expect(raceLabel.payload.raceId).toBe('race-1');
    }
  });
});
