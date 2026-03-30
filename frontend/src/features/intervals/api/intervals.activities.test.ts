import { describe, expect, it } from 'vitest';

import { deleteActivity, listActivities, loadActivity, updateActivity, uploadActivity } from './intervals';
import { createFetchMock, useFetchMock } from './testHelpers';

describe('intervals api activities', () => {
  it('lists, uploads, loads, updates and deletes activities', async () => {
    const fetchMock = useFetchMock(
      createFetchMock()
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
                  strainScore: 13.5,
                },
                details: {
                  intervals: [], intervalGroups: [], streams: [], intervalSummary: [], skylineChart: [], powerZoneTimes: [], heartRateZoneTimes: [], paceZoneTimes: [], gapZoneTimes: [],
                },
              },
            ]),
            { status: 200, headers: { 'content-type': 'application/json' } },
          ),
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
                  metrics: { trainingStressScore: null, normalizedPowerWatts: null, intensityFactor: null, efficiencyFactor: null, variabilityIndex: null, averagePowerWatts: null, ftpWatts: null, totalWorkJoules: null, calories: null, trimp: null, powerLoad: null, heartRateLoad: null, paceLoad: null, strainScore: null },
                  details: { intervals: [], intervalGroups: [], streams: [], intervalSummary: [], skylineChart: [], powerZoneTimes: [], heartRateZoneTimes: [], paceZoneTimes: [], gapZoneTimes: [] },
                },
              ],
            }),
            { status: 201, headers: { 'content-type': 'application/json' } },
          ),
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
              metrics: { trainingStressScore: null, normalizedPowerWatts: null, intensityFactor: null, efficiencyFactor: null, variabilityIndex: null, averagePowerWatts: null, ftpWatts: null, totalWorkJoules: null, calories: null, trimp: null, powerLoad: null, heartRateLoad: null, paceLoad: null, strainScore: null },
              details: { intervals: [], intervalGroups: [], streams: [], intervalSummary: [], skylineChart: [], powerZoneTimes: [], heartRateZoneTimes: [], paceZoneTimes: [], gapZoneTimes: [] },
            }),
            { status: 200, headers: { 'content-type': 'application/json' } },
          ),
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
              metrics: { trainingStressScore: null, normalizedPowerWatts: null, intensityFactor: null, efficiencyFactor: null, variabilityIndex: null, averagePowerWatts: null, ftpWatts: null, totalWorkJoules: null, calories: null, trimp: null, powerLoad: null, heartRateLoad: null, paceLoad: null, strainScore: null },
              details: { intervals: [], intervalGroups: [], streams: [], intervalSummary: [], skylineChart: [], powerZoneTimes: [], heartRateZoneTimes: [], paceZoneTimes: [], gapZoneTimes: [] },
            }),
            { status: 200, headers: { 'content-type': 'application/json' } },
          ),
        )
        .mockResolvedValueOnce(new Response(null, { status: 204 })),
    );

    const listed = await listActivities('', { oldest: '2026-03-01', newest: '2026-03-31' });
    const uploaded = await uploadActivity('', { filename: 'ride.fit', fileContentsBase64: 'AQID', name: 'Uploaded Ride' });
    const loaded = await loadActivity('', 'i2');
    const updated = await updateActivity('', 'i2', {
      name: 'Updated Ride',
      description: 'indoors',
      activityType: 'VirtualRide',
      trainer: true,
    });
    await deleteActivity('', 'i2');

    expect(listed[0].metrics.normalizedPowerWatts).toBe(238);
    expect(uploaded.created).toBe(true);
    expect(loaded.id).toBe('i2');
    expect(updated.name).toBe('Updated Ride');
    expect(fetchMock).toHaveBeenNthCalledWith(1, '/api/intervals/activities?oldest=2026-03-01&newest=2026-03-31', expect.objectContaining({ method: 'GET', credentials: 'include' }));
    expect(fetchMock).toHaveBeenNthCalledWith(2, '/api/intervals/activities', expect.objectContaining({
      method: 'POST',
      credentials: 'include',
      body: JSON.stringify({ filename: 'ride.fit', fileContentsBase64: 'AQID', name: 'Uploaded Ride' }),
    }));
    expect(fetchMock).toHaveBeenNthCalledWith(3, '/api/intervals/activities/i2', expect.objectContaining({ method: 'GET', credentials: 'include' }));
    expect(fetchMock).toHaveBeenNthCalledWith(4, '/api/intervals/activities/i2', expect.objectContaining({
      method: 'PUT',
      credentials: 'include',
      body: JSON.stringify({ name: 'Updated Ride', description: 'indoors', activityType: 'VirtualRide', trainer: true }),
    }));
    expect(fetchMock).toHaveBeenNthCalledWith(5, '/api/intervals/activities/i2', expect.objectContaining({ method: 'DELETE', credentials: 'include' }));
  });
});
