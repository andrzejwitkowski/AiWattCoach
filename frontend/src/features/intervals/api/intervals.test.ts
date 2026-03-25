import { afterEach, describe, expect, it, vi } from 'vitest';

import {
  createEvent,
  deleteActivity,
  deleteEvent,
  downloadFit,
  listActivities,
  listEvents,
  loadActivity,
  loadEvent,
  updateActivity,
  updateEvent,
  uploadActivity,
} from './intervals';
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

  it('lists, uploads, loads, updates and deletes activities', async () => {
    const fetchMock = vi
      .fn<(input: RequestInfo | URL, init?: RequestInit) => Promise<Response>>()
      .mockResolvedValueOnce(
        new Response(
          JSON.stringify([
            {
              id: 'i1',
              startDateLocal: '2026-03-25T08:00:00',
              startDate: '2026-03-25T07:00:00Z',
              name: 'Morning Ride',
              description: 'tempo',
              activityType: 'Ride',
              source: 'UPLOAD',
              externalId: 'ext-1',
              deviceName: 'Garmin',
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
              tags: ['tempo'],
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
                strainScore: 13.5
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
                gapZoneTimes: []
              }
            }
          ]),
          { status: 200, headers: { 'content-type': 'application/json' } }
        )
      )
      .mockResolvedValueOnce(
        new Response(
          JSON.stringify({
            created: true,
            activityIds: ['i2'],
            activities: [
              {
                id: 'i2',
                startDateLocal: '2026-03-26T08:00:00',
                startDate: null,
                name: 'Uploaded Ride',
                description: null,
                activityType: 'Ride',
                source: 'UPLOAD',
                externalId: null,
                deviceName: null,
                distanceMeters: null,
                movingTimeSeconds: null,
                elapsedTimeSeconds: null,
                totalElevationGainMeters: null,
                averageSpeedMps: null,
                averageHeartRateBpm: null,
                averageCadenceRpm: null,
                trainer: false,
                commute: false,
                race: false,
                hasHeartRate: false,
                streamTypes: [],
                tags: [],
                metrics: {
                  trainingStressScore: null,
                  normalizedPowerWatts: null,
                  intensityFactor: null,
                  efficiencyFactor: null,
                  variabilityIndex: null,
                  averagePowerWatts: null,
                  ftpWatts: null,
                  totalWorkJoules: null,
                  calories: null,
                  trimp: null,
                  powerLoad: null,
                  heartRateLoad: null,
                  paceLoad: null,
                  strainScore: null
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
                  gapZoneTimes: []
                }
              }
            ]
          }),
          { status: 201, headers: { 'content-type': 'application/json' } }
        )
      )
      .mockResolvedValueOnce(
        new Response(
          JSON.stringify({
            id: 'i2',
            startDateLocal: '2026-03-26T08:00:00',
            startDate: null,
            name: 'Uploaded Ride',
            description: null,
            activityType: 'Ride',
            source: 'UPLOAD',
            externalId: null,
            deviceName: null,
            distanceMeters: null,
            movingTimeSeconds: null,
            elapsedTimeSeconds: null,
            totalElevationGainMeters: null,
            averageSpeedMps: null,
            averageHeartRateBpm: null,
            averageCadenceRpm: null,
            trainer: false,
            commute: false,
            race: false,
            hasHeartRate: false,
            streamTypes: [],
            tags: [],
            metrics: {
              trainingStressScore: null,
              normalizedPowerWatts: null,
              intensityFactor: null,
              efficiencyFactor: null,
              variabilityIndex: null,
              averagePowerWatts: null,
              ftpWatts: null,
              totalWorkJoules: null,
              calories: null,
              trimp: null,
              powerLoad: null,
              heartRateLoad: null,
              paceLoad: null,
              strainScore: null
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
              gapZoneTimes: []
            }
          }),
          { status: 200, headers: { 'content-type': 'application/json' } }
        )
      )
      .mockResolvedValueOnce(
        new Response(
          JSON.stringify({
            id: 'i2',
            startDateLocal: '2026-03-26T08:00:00',
            startDate: null,
            name: 'Updated Ride',
            description: 'indoors',
            activityType: 'VirtualRide',
            source: 'UPLOAD',
            externalId: null,
            deviceName: null,
            distanceMeters: null,
            movingTimeSeconds: null,
            elapsedTimeSeconds: null,
            totalElevationGainMeters: null,
            averageSpeedMps: null,
            averageHeartRateBpm: null,
            averageCadenceRpm: null,
            trainer: true,
            commute: false,
            race: false,
            hasHeartRate: false,
            streamTypes: [],
            tags: [],
            metrics: {
              trainingStressScore: null,
              normalizedPowerWatts: null,
              intensityFactor: null,
              efficiencyFactor: null,
              variabilityIndex: null,
              averagePowerWatts: null,
              ftpWatts: null,
              totalWorkJoules: null,
              calories: null,
              trimp: null,
              powerLoad: null,
              heartRateLoad: null,
              paceLoad: null,
              strainScore: null
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
              gapZoneTimes: []
            }
          }),
          { status: 200, headers: { 'content-type': 'application/json' } }
        )
      )
      .mockResolvedValueOnce(new Response(null, { status: 204 }));

    global.fetch = fetchMock as typeof fetch;

    const listed = await listActivities('', { oldest: '2026-03-01', newest: '2026-03-31' });
    const uploaded = await uploadActivity('', {
      filename: 'ride.fit',
      fileContentsBase64: 'AQID',
      name: 'Uploaded Ride'
    });
    const loaded = await loadActivity('', 'i2');
    const updated = await updateActivity('', 'i2', {
      name: 'Updated Ride',
      description: 'indoors',
      activityType: 'VirtualRide',
      trainer: true
    });
    await deleteActivity('', 'i2');

    expect(listed[0].metrics.normalizedPowerWatts).toBe(238);
    expect(uploaded.created).toBe(true);
    expect(loaded.id).toBe('i2');
    expect(updated.name).toBe('Updated Ride');
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
