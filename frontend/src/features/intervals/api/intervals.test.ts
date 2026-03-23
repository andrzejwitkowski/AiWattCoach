import { afterEach, describe, expect, it, vi } from 'vitest';

import { createEvent, deleteEvent, downloadFit, listEvents, loadEvent, updateEvent } from './intervals';
import { AuthenticationError, HttpError } from '../../../lib/httpClient';

const originalFetch = global.fetch;

afterEach(() => {
  global.fetch = originalFetch;
  vi.restoreAllMocks();
});

describe('intervals api', () => {
  it('loads interval events with nested eventDefinition and actualWorkout', async () => {
    const fetchMock = vi
      .fn<(input: RequestInfo | URL, init?: RequestInit) => Promise<Response>>()
      .mockResolvedValue(
        new Response(
          JSON.stringify([
            {
              id: 1,
              startDateLocal: '2026-03-22',
              name: 'Workout',
              category: 'WORKOUT',
              description: 'desc',
              indoor: true,
              color: 'blue',
              eventDefinition: {
                rawWorkoutDoc: '- 10min 55%',
                intervals: [{ definition: '- 10min 55%' }]
              },
              actualWorkout: null
            }
          ]),
          {
            status: 200,
            headers: { 'content-type': 'application/json' }
          }
        )
      );

    global.fetch = fetchMock as typeof fetch;

    const result = await listEvents('', {
      oldest: '2026-03-01',
      newest: '2026-03-31'
    });

    expect(fetchMock).toHaveBeenCalledWith(
      '/api/intervals/events?oldest=2026-03-01&newest=2026-03-31',
      {
        method: 'GET',
        headers: { Accept: 'application/json' },
        credentials: 'include',
        body: undefined
      }
    );
    expect(result[0].eventDefinition.intervals[0].definition).toBe('- 10min 55%');
  });

  it('creates, updates, loads and deletes interval events', async () => {
    const fetchMock = vi
      .fn<(input: RequestInfo | URL, init?: RequestInit) => Promise<Response>>()
      .mockResolvedValueOnce(
        new Response(
          JSON.stringify({
            id: 2,
            startDateLocal: '2026-03-25',
            name: 'Created',
            category: 'WORKOUT',
            description: null,
            indoor: true,
            color: 'green',
            eventDefinition: {
              rawWorkoutDoc: '- 2x20min 90%',
              intervals: [{ definition: '- 2x20min 90%' }]
            },
            actualWorkout: null
          }),
          {
            status: 201,
            headers: { 'content-type': 'application/json' }
          }
        )
      )
      .mockResolvedValueOnce(
        new Response(
          JSON.stringify({
            id: 2,
            startDateLocal: '2026-03-25',
            name: 'Updated',
            category: 'WORKOUT',
            description: null,
            indoor: false,
            color: null,
            eventDefinition: {
              rawWorkoutDoc: '- 3x8min 100%',
              intervals: [{ definition: '- 3x8min 100%' }]
            },
            actualWorkout: null
          }),
          {
            status: 200,
            headers: { 'content-type': 'application/json' }
          }
        )
      )
      .mockResolvedValueOnce(
        new Response(
          JSON.stringify({
            id: 2,
            startDateLocal: '2026-03-25',
            name: 'Updated',
            category: 'WORKOUT',
            description: null,
            indoor: false,
            color: null,
            eventDefinition: {
              rawWorkoutDoc: '- 3x8min 100%',
              intervals: [{ definition: '- 3x8min 100%' }]
            },
            actualWorkout: null
          }),
          {
            status: 200,
            headers: { 'content-type': 'application/json' }
          }
        )
      )
      .mockResolvedValueOnce(
        new Response(null, { status: 204 })
      )
      .mockResolvedValueOnce(new Response(new Uint8Array([1, 2, 3]), { status: 200 }))
      .mockResolvedValueOnce(
        new Response(
          JSON.stringify({
            id: 2,
            startDateLocal: '2026-03-25',
            name: 'Updated',
            category: 'WORKOUT',
            description: null,
            indoor: false,
            color: null,
            eventDefinition: { intervals: [] },
            actualWorkout: {
              powerValues: [],
              cadenceValues: [],
              heartRateValues: []
            }
          }),
          {
            status: 200,
            headers: { 'content-type': 'application/json' }
          }
        )
      );

    global.fetch = fetchMock as typeof fetch;

    const created = await createEvent('', {
      category: 'WORKOUT',
      startDateLocal: '2026-03-25',
      name: 'Created',
      indoor: true,
      color: 'green',
      workoutDoc: '- 2x20min 90%'
    });
    const updated = await updateEvent('', 2, {
      name: 'Updated',
      indoor: false,
      workoutDoc: '- 3x8min 100%'
    });
    const loaded = await loadEvent('', 2);
    await deleteEvent('', 2);
    const fitBytes = await downloadFit('', 2);

    expect(created.name).toBe('Created');
    expect(updated.name).toBe('Updated');
    expect(loaded.id).toBe(2);
    expect(Array.from(fitBytes)).toEqual([1, 2, 3]);
  });

  it('throws shared client errors for fit download failures', async () => {
    const unauthorizedFetch = vi
      .fn<(input: RequestInfo | URL, init?: RequestInit) => Promise<Response>>()
      .mockResolvedValueOnce(new Response(null, { status: 401 }));

    global.fetch = unauthorizedFetch as typeof fetch;
    await expect(downloadFit('', 4)).rejects.toBeInstanceOf(AuthenticationError);

    const failingFetch = vi
      .fn<(input: RequestInfo | URL, init?: RequestInit) => Promise<Response>>()
      .mockResolvedValueOnce(new Response(null, { status: 502 }));

    global.fetch = failingFetch as typeof fetch;
    await expect(downloadFit('', 4)).rejects.toBeInstanceOf(HttpError);
  });
});
