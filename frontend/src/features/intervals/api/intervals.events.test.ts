import { describe, expect, it } from 'vitest';

import { AuthenticationError, HttpError } from '../../../lib/httpClient';
import { createEvent, deleteEvent, downloadFit, listEvents, loadEvent, updateEvent } from './intervals';
import { createFetchMock, useFetchMock } from './testHelpers';

describe('intervals api events', () => {
  it('loads interval events with nested eventDefinition and actualWorkout', async () => {
    const fetchMock = useFetchMock(
      createFetchMock().mockResolvedValue(
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
                intervals: [{ definition: '- 10min 55%', repeatCount: 1, durationSeconds: 600, targetPercentFtp: 55, zoneId: 1 }],
                segments: [{ order: 0, label: '10min 55%', durationSeconds: 600, startOffsetSeconds: 0, endOffsetSeconds: 600, targetPercentFtp: 55, zoneId: 1 }],
                summary: { totalSegments: 1, totalDurationSeconds: 600, estimatedNormalizedPowerWatts: null, estimatedAveragePowerWatts: null, estimatedIntensityFactor: 0.55, estimatedTrainingStressScore: 5 },
              },
              actualWorkout: null,
            },
          ]),
          { status: 200, headers: { 'content-type': 'application/json' } },
        ),
      ),
    );

    const result = await listEvents('', { oldest: '2026-03-01', newest: '2026-03-31' });

    expect(fetchMock).toHaveBeenCalledWith('/api/intervals/events?oldest=2026-03-01&newest=2026-03-31', {
      method: 'GET',
      headers: {
        Accept: 'application/json',
        traceparent: expect.stringMatching(/^[0-9a-f]{2}-[0-9a-f]{32}-[0-9a-f]{16}-[0-9a-f]{2}$/),
      },
      credentials: 'include',
      body: undefined,
    });
    expect(result[0].eventDefinition.intervals[0].definition).toBe('- 10min 55%');
    expect(result[0].eventDefinition.segments[0].durationSeconds).toBe(600);
    expect(result[0].eventDefinition.summary.estimatedIntensityFactor).toBe(0.55);
  });

  it('creates, updates, loads and deletes interval events', async () => {
    useFetchMock(
      createFetchMock()
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
                intervals: [{ definition: '- 2x20min 90%', repeatCount: 2, durationSeconds: 1200, targetPercentFtp: 90, zoneId: 3 }],
                segments: [],
                summary: { totalSegments: 2, totalDurationSeconds: 2400, estimatedNormalizedPowerWatts: null, estimatedAveragePowerWatts: null, estimatedIntensityFactor: 0.9, estimatedTrainingStressScore: 54 },
              },
              actualWorkout: null,
            }),
            { status: 201, headers: { 'content-type': 'application/json' } },
          ),
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
                intervals: [{ definition: '- 3x8min 100%', repeatCount: 3, durationSeconds: 480, targetPercentFtp: 100, zoneId: 4 }],
                segments: [],
                summary: { totalSegments: 3, totalDurationSeconds: 1440, estimatedNormalizedPowerWatts: null, estimatedAveragePowerWatts: null, estimatedIntensityFactor: 1, estimatedTrainingStressScore: 40 },
              },
              actualWorkout: null,
            }),
            { status: 200, headers: { 'content-type': 'application/json' } },
          ),
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
                intervals: [{ definition: '- 3x8min 100%', repeatCount: 3, durationSeconds: 480, targetPercentFtp: 100, zoneId: 4 }],
                segments: [],
                summary: { totalSegments: 3, totalDurationSeconds: 1440, estimatedNormalizedPowerWatts: null, estimatedAveragePowerWatts: null, estimatedIntensityFactor: 1, estimatedTrainingStressScore: 40 },
              },
              actualWorkout: null,
            }),
            { status: 200, headers: { 'content-type': 'application/json' } },
          ),
        )
        .mockResolvedValueOnce(new Response(null, { status: 204 }))
        .mockResolvedValueOnce(new Response(new Uint8Array([1, 2, 3]), { status: 200 })),
    );

    const created = await createEvent('', {
      category: 'WORKOUT',
      startDateLocal: '2026-03-25',
      name: 'Created',
      indoor: true,
      color: 'green',
      workoutDoc: '- 2x20min 90%',
    });
    const updated = await updateEvent('', 2, {
      name: 'Updated',
      indoor: false,
      workoutDoc: '- 3x8min 100%',
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
    useFetchMock(createFetchMock().mockResolvedValueOnce(new Response(null, { status: 401 })));
    await expect(downloadFit('', 4)).rejects.toBeInstanceOf(AuthenticationError);

    useFetchMock(createFetchMock().mockResolvedValueOnce(new Response(null, { status: 502 })));
    await expect(downloadFit('', 4)).rejects.toBeInstanceOf(HttpError);
  });
});
