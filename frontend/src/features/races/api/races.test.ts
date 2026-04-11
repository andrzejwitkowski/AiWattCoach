import { describe, expect, it } from 'vitest';

import { createRace, deleteRace, getRace, listRaces, updateRace } from './races';
import { createFetchMock, useFetchMock } from '../../intervals/api/testHelpers';

const raceFixture = {
  raceId: 'race-2',
  date: '2026-09-20',
  name: 'Road Challenge',
  distanceMeters: 90000,
  discipline: 'road',
  priority: 'A',
  syncStatus: 'pending',
  linkedIntervalsEventId: null,
  lastError: null,
};

const updatedRaceFixture = {
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
};

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
    expect(result).toEqual([
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
    ]);
  });

  it('creates, loads, updates, and deletes races', async () => {
    const fetchMock = useFetchMock(
      createFetchMock()
        .mockResolvedValueOnce(
          new Response(
            JSON.stringify(raceFixture),
            { status: 201, headers: { 'content-type': 'application/json' } },
          ),
        )
        .mockResolvedValueOnce(
          new Response(
            JSON.stringify(raceFixture),
            { status: 200, headers: { 'content-type': 'application/json' } },
          ),
        )
        .mockResolvedValueOnce(
          new Response(
            JSON.stringify(updatedRaceFixture),
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

    expect(fetchMock).toHaveBeenNthCalledWith(1, '/api/races', {
      method: 'POST',
      headers: expect.objectContaining({
        Accept: 'application/json',
        'Content-Type': 'application/json',
        traceparent: expect.stringMatching(/^[0-9a-f]{2}-[0-9a-f]{32}-[0-9a-f]{16}-[0-9a-f]{2}$/),
      }),
      credentials: 'include',
      body: JSON.stringify({
        date: '2026-09-20',
        name: 'Road Challenge',
        distanceMeters: 90000,
        discipline: 'road',
        priority: 'A',
      }),
    });
    expect(fetchMock).toHaveBeenNthCalledWith(2, '/api/races/race-2', {
      method: 'GET',
      headers: expect.objectContaining({
        Accept: 'application/json',
        traceparent: expect.stringMatching(/^[0-9a-f]{2}-[0-9a-f]{32}-[0-9a-f]{16}-[0-9a-f]{2}$/),
      }),
      credentials: 'include',
      body: undefined,
    });
    expect(fetchMock).toHaveBeenNthCalledWith(3, '/api/races/race-2', {
      method: 'PUT',
      headers: expect.objectContaining({
        Accept: 'application/json',
        'Content-Type': 'application/json',
        traceparent: expect.stringMatching(/^[0-9a-f]{2}-[0-9a-f]{32}-[0-9a-f]{16}-[0-9a-f]{2}$/),
      }),
      credentials: 'include',
      body: JSON.stringify({
        date: '2026-09-21',
        name: 'Road Challenge Updated',
        distanceMeters: 95000,
        discipline: 'road',
        priority: 'B',
      }),
    });
    expect(fetchMock).toHaveBeenNthCalledWith(4, '/api/races/race-2', {
      method: 'DELETE',
      headers: expect.objectContaining({
        Accept: 'application/json',
        traceparent: expect.stringMatching(/^[0-9a-f]{2}-[0-9a-f]{32}-[0-9a-f]{16}-[0-9a-f]{2}$/),
      }),
      credentials: 'include',
      body: undefined,
    });
    expect(created).toEqual(raceFixture);
    expect(loaded).toEqual(raceFixture);
    expect(updated).toEqual(updatedRaceFixture);
  });

  it('rejects malformed race payloads at the zod boundary', async () => {
    useFetchMock(
      createFetchMock().mockResolvedValueOnce(
        new Response(
          JSON.stringify({
            raceId: 'race-2',
            date: '2026-09-20',
            name: 'Road Challenge',
            distanceMeters: 90000,
            discipline: 'track',
            priority: 'A',
            syncStatus: 'pending',
            linkedIntervalsEventId: null,
            lastError: null,
          }),
          { status: 200, headers: { 'content-type': 'application/json' } },
        ),
      ),
    );

    await expect(getRace('', 'race-2')).rejects.toThrow();
  });
});
