import { describe, expect, it } from 'vitest';

import type { IntervalActivity, IntervalEvent } from '../intervals/types';
import { buildCompletedWorkoutBars, buildPlannedWorkoutBars, formatDurationLabel } from './workoutDetails';

describe('workoutDetails', () => {
  it('builds planned bars from parsed workout segments with zone colors', () => {
    const event: IntervalEvent = {
      id: 1,
      startDateLocal: '2026-03-22',
      name: 'VO2 Session',
      category: 'WORKOUT',
      description: null,
      indoor: true,
      color: null,
      eventDefinition: {
        rawWorkoutDoc: '- 4x4min 110%',
        intervals: [
          {
            definition: '- 4x4min 110%',
            repeatCount: 4,
            durationSeconds: 240,
            targetPercentFtp: 110,
            zoneId: 5,
          },
        ],
        segments: Array.from({ length: 4 }, (_, index) => ({
          order: index,
          label: `4x4min 110% #${index + 1}`,
          durationSeconds: 240,
          startOffsetSeconds: index * 240,
          endOffsetSeconds: (index + 1) * 240,
          targetPercentFtp: 110,
          zoneId: 5,
        })),
        summary: {
          totalSegments: 4,
          totalDurationSeconds: 960,
          estimatedNormalizedPowerWatts: null,
          estimatedAveragePowerWatts: null,
          estimatedIntensityFactor: 1.1,
          estimatedTrainingStressScore: 29.3,
        },
      },
      actualWorkout: null,
    };

    const bars = buildPlannedWorkoutBars(event);

    expect(bars).toHaveLength(4);
    expect(bars[0]).toEqual({ height: 100, color: '#facc15' });
  });

  it('builds completed bars from actual intervals before falling back to streams', () => {
    const activity: IntervalActivity = {
      id: 'a1',
      startDateLocal: '2026-03-22T08:00:00',
      startDate: '2026-03-22T07:00:00Z',
      name: 'Outside Threshold',
      description: null,
      activityType: 'Ride',
      source: null,
      externalId: null,
      deviceName: null,
      distanceMeters: 40000,
      movingTimeSeconds: 3600,
      elapsedTimeSeconds: 3700,
      totalElevationGainMeters: null,
      averageSpeedMps: null,
      averageHeartRateBpm: null,
      averageCadenceRpm: null,
      trainer: false,
      commute: false,
      race: false,
      hasHeartRate: true,
      streamTypes: ['watts'],
      tags: [],
      metrics: {
        trainingStressScore: 82,
        normalizedPowerWatts: 265,
        intensityFactor: 0.88,
        efficiencyFactor: null,
        variabilityIndex: null,
        averagePowerWatts: 240,
        ftpWatts: 300,
        totalWorkJoules: null,
        calories: null,
        trimp: null,
        powerLoad: null,
        heartRateLoad: null,
        paceLoad: null,
        strainScore: null,
      },
      details: {
        intervals: [
          {
            id: 1,
            label: 'Work 1',
            intervalType: 'WORK',
            groupId: null,
            startIndex: 0,
            endIndex: 1,
            startTimeSeconds: 600,
            endTimeSeconds: 960,
            movingTimeSeconds: 360,
            elapsedTimeSeconds: 360,
            distanceMeters: null,
            averagePowerWatts: 285,
            normalizedPowerWatts: 288,
            trainingStressScore: null,
            averageHeartRateBpm: null,
            averageCadenceRpm: null,
            averageSpeedMps: null,
            averageStrideMeters: null,
            zone: 4,
          },
        ],
        intervalGroups: [],
        streams: [
          {
            streamType: 'watts',
            name: 'Power',
            data: [100, 110, 120],
            data2: null,
            valueTypeIsArray: false,
            custom: false,
            allNull: false,
          },
        ],
        intervalSummary: [],
        skylineChart: [],
        powerZoneTimes: [],
        heartRateZoneTimes: [],
        paceZoneTimes: [],
        gapZoneTimes: [],
      },
    };

    const bars = buildCompletedWorkoutBars(activity);

    expect(bars).toHaveLength(1);
    expect(bars[0]).toEqual({ height: 71, color: '#d2ff9a' });
  });

  it('formats compact duration labels', () => {
    expect(formatDurationLabel(3900)).toBe('1h 05m');
    expect(formatDurationLabel(1500)).toBe('25m');
  });
});
