import { describe, expect, it } from 'vitest';

import { createRace, deleteRace, getRace, listRaces, updateRace } from './races';
import { createFetchMock, useFetchMock } from '../../intervals/api/testHelpers';

describe('races api', () => {
  it('lists races for a date range', async () => {
    const fetchMock = useFetchMock(
      createFetchMock().mockResolvedValueOnce(
        new Response(
          JSON.stringify([
            {
              raceId: 'race-1',
              date: '2026-09-12',
              name: 'Gravel Attack',
              distanceMeters: 120000,
              discipline: 'gravel',
              priority: 'B',
              syncStatus: 'synced',
              linkedIntervalsEventId: 91,
              lastError: null,
            },
          ]),
          { status: 200, headers: { 'content-type': 'application/json' } },
        ),
      ),
    );

    const result = await listRaces('', { oldest: '2026-09-01', newest: '2026-09-30' });

    expect(fetchMock).toHaveBeenCalledWith('/api/races?oldest=2026-09-01&newest=2026-09-30', {
      method: 'GET',
      headers: {
        Accept: 'application/json',
        traceparent: expect.stringMatching(/^[0-9a-f]{2}-[0-9a-f]{32}-[0-9a-f]{16}-[0-9a-f]{2}$/),
      },
      credentials: 'include',
      body: undefined,
    });
    expect(result[0]?.name).toBe('Gravel Attack');
    expect(result[0]?.discipline).toBe('gravel');
  });

  it('creates, loads, updates, and deletes races', async () => {
    useFetchMock(
      createFetchMock()
        .mockResolvedValueOnce(
          new Response(
            JSON.stringify({
              raceId: 'race-2',
              date: '2026-09-20',
              name: 'Road Challenge',
              distanceMeters: 90000,
              discipline: 'road',
              priority: 'A',
              syncStatus: 'pending',
              linkedIntervalsEventId: null,
              lastError: null,
            }),
            { status: 201, headers: { 'content-type': 'application/json' } },
          ),
        )
        .mockResolvedValueOnce(
          new Response(
            JSON.stringify({
              raceId: 'race-2',
              date: '2026-09-20',
              name: 'Road Challenge',
              distanceMeters: 90000,
              discipline: 'road',
              priority: 'A',
              syncStatus: 'pending',
              linkedIntervalsEventId: null,
              lastError: null,
            }),
            { status: 200, headers: { 'content-type': 'application/json' } },
          ),
        )
        .mockResolvedValueOnce(
          new Response(
            JSON.stringify({
              raceId: 'race-2',
              date: '2026-09-21',
              name: 'Road Challenge Updated',
              distanceMeters: 95000,
              discipline: 'road',
              priority: 'B',
              syncStatus: 'synced',
              linkedIntervalsEventId: 404,
              lastError: null,
              result: 'finished',
            }),
            { status: 200, headers: { 'content-type': 'application/json' } },
          ),
        )
        .mockResolvedValueOnce(new Response(null, { status: 204 })),
    );

    const created = await createRace('', {
      date: '2026-09-20',
      name: 'Road Challenge',
      distanceMeters: 90000,
      discipline: 'road',
      priority: 'A',
    });
    const loaded = await getRace('', 'race-2');
    const updated = await updateRace('', 'race-2', {
      date: '2026-09-21',
      name: 'Road Challenge Updated',
      distanceMeters: 95000,
      discipline: 'road',
      priority: 'B',
    });
    await deleteRace('', 'race-2');

    expect(created.raceId).toBe('race-2');
    expect(loaded.priority).toBe('A');
    expect(updated.result).toBe('finished');
    expect(updated.linkedIntervalsEventId).toBe(404);
  });
});
