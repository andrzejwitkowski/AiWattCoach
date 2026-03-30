import { describe, expect, it } from 'vitest';

import type { IntervalActivity, IntervalEvent } from '../intervals/types';
import { buildCompletedWorkoutBars, buildMatchedWorkoutBars, buildPlannedWorkoutBars, extractCompletedPowerValues, formatDurationLabel } from './workoutDetails';

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
    expect(bars[0]).toEqual({ height: 100, color: '#facc15', widthUnits: 240 });
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
      detailsUnavailableReason: null,
    };

    const bars = buildCompletedWorkoutBars(activity);

    expect(bars).toHaveLength(1);
    expect(bars[0]).toEqual({ height: 71, color: '#d2ff9a', widthUnits: 360 });
  });

  it('builds matched workout bars with width proportional to interval duration', () => {
    const bars = buildMatchedWorkoutBars({
      activityId: 'a3',
      activityName: 'Threshold Ride',
      startDateLocal: '2026-03-25T08:00:00',
      powerValues: [150, 210, 290, 275],
      cadenceValues: [85, 88, 92, 89],
      heartRateValues: [130, 145, 162, 165],
      speedValues: [8.2, 9.5, 10.4, 10.1],
      averagePowerWatts: 271,
      normalizedPowerWatts: 280,
      trainingStressScore: 35,
      intensityFactor: 0.93,
      complianceScore: 0.91,
      matchedIntervals: [
        {
          plannedSegmentOrder: 0,
          plannedLabel: 'Long Block',
          plannedDurationSeconds: 1200,
          targetPercentFtp: 95,
          zoneId: 4,
          actualIntervalId: 1,
          actualStartTimeSeconds: 0,
          actualEndTimeSeconds: 1200,
          averagePowerWatts: 271,
          normalizedPowerWatts: 280,
          averageHeartRateBpm: 161,
          averageCadenceRpm: 89,
          averageSpeedMps: 10.1,
          complianceScore: 0.91,
        },
        {
          plannedSegmentOrder: 1,
          plannedLabel: 'Short Block',
          plannedDurationSeconds: 300,
          targetPercentFtp: 85,
          zoneId: 3,
          actualIntervalId: 2,
          actualStartTimeSeconds: 1200,
          actualEndTimeSeconds: 1500,
          averagePowerWatts: 240,
          normalizedPowerWatts: 245,
          averageHeartRateBpm: 150,
          averageCadenceRpm: 86,
          averageSpeedMps: 9.4,
          complianceScore: 0.88,
        },
      ],
    });

    expect(bars).toEqual([
      { height: 68, color: '#d2ff9a', widthUnits: 1200 },
      { height: 60, color: '#52c41a', widthUnits: 300 },
    ]);
  });

  it('formats compact duration labels', () => {
    expect(formatDurationLabel(3900)).toBe('1h 05m');
    expect(formatDurationLabel(1500)).toBe('25m');
  });

  it('extracts completed power values from watts streams', () => {
    const activity: IntervalActivity = {
      id: 'a2',
      startDateLocal: '2026-03-22T08:00:00',
      startDate: '2026-03-22T07:00:00Z',
      name: 'Outside Endurance',
      description: null,
      activityType: 'Ride',
      source: null,
      externalId: null,
      deviceName: null,
      distanceMeters: 32000,
      movingTimeSeconds: 3600,
      elapsedTimeSeconds: 3610,
      totalElevationGainMeters: null,
      averageSpeedMps: null,
      averageHeartRateBpm: null,
      averageCadenceRpm: null,
      trainer: false,
      commute: false,
      race: false,
      hasHeartRate: true,
      streamTypes: ['watts', 'time'],
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
        strainScore: null,
      },
      details: {
        intervals: [],
        intervalGroups: [],
        streams: [
          {
            streamType: 'time',
            name: null,
            data: [0, 1, 2, 3],
            data2: null,
            valueTypeIsArray: false,
            custom: false,
            allNull: false,
          },
          {
            streamType: 'watts',
            name: 'Power',
            data: [198.4, 210.2, 222.7, 219.9],
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
      detailsUnavailableReason: null,
    };

    expect(extractCompletedPowerValues(activity)).toEqual([198, 210, 223, 220]);
  });
});
