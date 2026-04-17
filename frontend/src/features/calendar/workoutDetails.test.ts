import { describe, expect, it } from 'vitest';

import type { IntervalActivity, IntervalEvent } from '../intervals/types';
import { buildCompletedWorkoutBars, buildCompletedWorkoutPreviewBars, buildFiveSecondAveragePowerSeries, buildMatchedWorkoutBars, buildPlannedWorkoutBars, buildPlannedWorkoutChartIntervals, buildPlannedWorkoutPowerSeries, buildPlannedWorkoutStructureItems, buildPlannedWorkoutStructureSections, extractCompletedPowerValues, formatDurationLabel, formatPlannedWorkoutIntervalLabel, isPlannedWorkoutEvent, selectWorkoutDetail } from './workoutDetails';

describe('workoutDetails', () => {
  it('builds planned bars from parsed workout segments with zone colors', () => {
    const event: IntervalEvent = {
      id: 1,
      startDateLocal: '2026-03-22',
      name: 'VO2 Session',
      category: 'WORKOUT',
      description: null,
      restDay: false,
      restDayReason: null,
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

  it('builds interval-only planned bars from target intensity instead of index order', () => {
    const event: IntervalEvent = {
      id: 17,
      startDateLocal: '2026-03-22',
      name: 'Steady Builder',
      category: 'WORKOUT',
      description: null,
      restDay: false,
      restDayReason: null,
      indoor: true,
      color: null,
      eventDefinition: {
        rawWorkoutDoc: '- 2x20min 90%',
        intervals: [
          {
            definition: '- 2x20min 90%',
            repeatCount: 2,
            durationSeconds: 1200,
            targetPercentFtp: 90,
            zoneId: 3,
          },
        ],
        segments: [],
        summary: {
          totalSegments: 1,
          totalDurationSeconds: 2400,
          estimatedNormalizedPowerWatts: null,
          estimatedAveragePowerWatts: null,
          estimatedIntensityFactor: null,
          estimatedTrainingStressScore: null,
        },
      },
      actualWorkout: null,
    };

    expect(buildPlannedWorkoutBars(event)).toEqual([
      { height: 90, color: '#52c41a', widthUnits: 2400 },
    ]);
  });

  it('uses zone fallback target heights for planned bars when ftp percent is missing', () => {
    const event: IntervalEvent = {
      id: 18,
      startDateLocal: '2026-03-22',
      name: 'Zone Builder',
      category: 'WORKOUT',
      description: null,
      restDay: false,
      restDayReason: null,
      indoor: true,
      color: null,
      eventDefinition: {
        rawWorkoutDoc: null,
        intervals: [
          {
            definition: '10min z2',
            repeatCount: 1,
            durationSeconds: 600,
            targetPercentFtp: null,
            zoneId: 2,
          },
        ],
        segments: [
          {
            order: 0,
            label: 'Zone 2',
            durationSeconds: 600,
            startOffsetSeconds: 0,
            endOffsetSeconds: 600,
            targetPercentFtp: null,
            zoneId: 2,
          },
        ],
        summary: {
          totalSegments: 1,
          totalDurationSeconds: 600,
          estimatedNormalizedPowerWatts: null,
          estimatedAveragePowerWatts: null,
          estimatedIntensityFactor: null,
          estimatedTrainingStressScore: null,
        },
      },
      actualWorkout: null,
    };

    expect(buildPlannedWorkoutBars(event)).toEqual([
      { height: 70, color: '#00e3fd', widthUnits: 600 },
    ]);
  });

  it('renders neutral planned bars when both target percent and zone are unknown', () => {
    const event: IntervalEvent = {
      id: 20,
      startDateLocal: '2026-03-22',
      name: 'Unknown Build',
      category: 'WORKOUT',
      description: null,
      restDay: false,
      restDayReason: null,
      indoor: true,
      color: null,
      eventDefinition: {
        rawWorkoutDoc: null,
        intervals: [
          {
            definition: '',
            repeatCount: 1,
            durationSeconds: 600,
            targetPercentFtp: null,
            zoneId: null,
          },
        ],
        segments: [],
        summary: {
          totalSegments: 1,
          totalDurationSeconds: 600,
          estimatedNormalizedPowerWatts: null,
          estimatedAveragePowerWatts: null,
          estimatedIntensityFactor: null,
          estimatedTrainingStressScore: null,
        },
      },
      actualWorkout: null,
    };

    expect(buildPlannedWorkoutBars(event)).toEqual([
      { height: 45, color: '#6b7280', widthUnits: 600 },
    ]);
  });

  it('derives planned bar color from ftp target when zone is missing', () => {
    const event: IntervalEvent = {
      id: 21,
      startDateLocal: '2026-03-22',
      name: 'Target Build',
      category: 'WORKOUT',
      description: null,
      restDay: false,
      restDayReason: null,
      indoor: true,
      color: null,
      eventDefinition: {
        rawWorkoutDoc: null,
        intervals: [
          {
            definition: '',
            repeatCount: 1,
            durationSeconds: 300,
            targetPercentFtp: 115,
            zoneId: null,
          },
        ],
        segments: [],
        summary: {
          totalSegments: 1,
          totalDurationSeconds: 300,
          estimatedNormalizedPowerWatts: null,
          estimatedAveragePowerWatts: null,
          estimatedIntensityFactor: null,
          estimatedTrainingStressScore: null,
        },
      },
      actualWorkout: null,
    };

    expect(buildPlannedWorkoutBars(event)).toEqual([
      { height: 100, color: '#facc15', widthUnits: 300 },
    ]);
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

  it('builds completed preview bars from skyline chart data before generic fallbacks', () => {
    const activity: IntervalActivity = {
      id: 'a-skyline',
      startDateLocal: '2026-03-22T08:00:00',
      startDate: '2026-03-22T07:00:00Z',
      name: 'Encoded Skyline Ride',
      description: null,
      activityType: 'Ride',
      source: null,
      externalId: null,
      deviceName: null,
      distanceMeters: 40000,
      movingTimeSeconds: 3600,
      elapsedTimeSeconds: 3600,
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
        trainingStressScore: 60,
        normalizedPowerWatts: 240,
        intensityFactor: 0.84,
        efficiencyFactor: null,
        variabilityIndex: null,
        averagePowerWatts: 228,
        ftpWatts: 283,
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
        streams: [],
        intervalSummary: [],
        skylineChart: ['CAcSAtJFGgFAIgECKAE='],
        powerZoneTimes: [],
        heartRateZoneTimes: [],
        paceZoneTimes: [],
        gapZoneTimes: [],
      },
      detailsUnavailableReason: null,
    };

    const bars = buildCompletedWorkoutPreviewBars(activity);

    expect(bars).toEqual([
      { height: 64, color: '#00e3fd', widthUnits: 82 },
    ]);
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

  it('formats planned interval labels with grouped repeat structure', () => {
    expect(
      formatPlannedWorkoutIntervalLabel({
        definition: '- 4x120% ftp 2min and 2min of rest 50%',
        repeatCount: 4,
        durationSeconds: 240,
        targetPercentFtp: 120,
        zoneId: 5,
      }),
    ).toBe('4 x 120% FTP 2min and 2min of rest 50% FTP');
  });

  it('formats planned interval labels without inventing a zero duration', () => {
    expect(
      formatPlannedWorkoutIntervalLabel({
        definition: '',
        repeatCount: 1,
        durationSeconds: null,
        targetPercentFtp: 90,
        zoneId: null,
      }),
    ).toBe('90% FTP');
  });

  it('builds planned structure items from interval definitions before raw text fallback', () => {
    const event: IntervalEvent = {
      id: 99,
      startDateLocal: '2026-03-30',
      name: 'Builder',
      category: 'WORKOUT',
      description: null,
      restDay: false,
      restDayReason: null,
      indoor: false,
      color: null,
      eventDefinition: {
        rawWorkoutDoc: '- 2x20min 90%',
        intervals: [
          {
            definition: '- 2x20min 90%',
            repeatCount: 2,
            durationSeconds: 1200,
            targetPercentFtp: 90,
            zoneId: 3,
          },
        ],
        segments: [],
        summary: {
          totalSegments: 1,
          totalDurationSeconds: 2400,
          estimatedNormalizedPowerWatts: null,
          estimatedAveragePowerWatts: null,
          estimatedIntensityFactor: null,
          estimatedTrainingStressScore: null,
        },
      },
      actualWorkout: null,
    };

    expect(buildPlannedWorkoutStructureItems(event)).toEqual([
      {
        id: 'interval-0',
        label: '2 x 20min 90% FTP',
        detail: '40m',
        durationSeconds: 2400,
      },
    ]);
  });

  it('builds grouped planned workout sections from raw workout headings', () => {
    const event: IntervalEvent = {
      id: 199,
      startDateLocal: '2026-04-28',
      name: 'Mixed Intervals',
      category: 'WORKOUT',
      description: null,
      restDay: false,
      restDayReason: null,
      indoor: false,
      color: null,
      eventDefinition: {
        rawWorkoutDoc: 'Mixed Intervals\nWarmup\n- 20m ramp 50-75%\nMain Set 4x\n- 5m 105%\n- 3m 55%\nMain Set 2 10x\n- 30s 130%\n- 30s 50%\nCooldown\n- 10m 50%',
        intervals: [
          {
            definition: '- 20m ramp 50-75%',
            repeatCount: 1,
            durationSeconds: 1200,
            targetPercentFtp: 62.5,
            zoneId: 2,
          },
          {
            definition: '- 5m 105%',
            repeatCount: 1,
            durationSeconds: 300,
            targetPercentFtp: 105,
            zoneId: 4,
          },
          {
            definition: '- 3m 55%',
            repeatCount: 1,
            durationSeconds: 180,
            targetPercentFtp: 55,
            zoneId: 1,
          },
          {
            definition: '- 30s 130%',
            repeatCount: 1,
            durationSeconds: 30,
            targetPercentFtp: 130,
            zoneId: 6,
          },
          {
            definition: '- 30s 50%',
            repeatCount: 1,
            durationSeconds: 30,
            targetPercentFtp: 50,
            zoneId: 1,
          },
          {
            definition: '- 10m 50%',
            repeatCount: 1,
            durationSeconds: 600,
            targetPercentFtp: 50,
            zoneId: 1,
          },
        ],
        segments: [
          {
            order: 0,
            label: 'Mixed Intervals',
            durationSeconds: 1,
            startOffsetSeconds: 0,
            endOffsetSeconds: 1,
            targetPercentFtp: null,
            zoneId: null,
          },
        ],
        summary: {
          totalSegments: 1,
          totalDurationSeconds: 2340,
          estimatedNormalizedPowerWatts: null,
          estimatedAveragePowerWatts: null,
          estimatedIntensityFactor: null,
          estimatedTrainingStressScore: null,
        },
      },
      actualWorkout: null,
    };

    expect(buildPlannedWorkoutStructureSections(event)).toEqual([
      {
        id: 'section-0',
        label: 'Warmup',
        repeatCount: 1,
        durationSeconds: 1200,
        steps: [
          {
            id: 'section-step-0',
            label: '20m ramp 50-75% FTP',
            detail: '20m • 62.5% FTP',
            durationSeconds: 1200,
            targetPercentFtp: 62.5,
            zoneId: 2,
          },
        ],
        targetPercentFtp: 62.5,
        zoneId: 2,
      },
      {
        id: 'section-1',
        label: 'Main Set 4x',
        repeatCount: 4,
        durationSeconds: 1920,
        steps: [
          {
            id: 'section-step-1',
            label: '5m 105% FTP',
            detail: '5m • 105% FTP',
            durationSeconds: 300,
            targetPercentFtp: 105,
            zoneId: 4,
          },
          {
            id: 'section-step-2',
            label: '3m 55% FTP',
            detail: '3m • 55% FTP',
            durationSeconds: 180,
            targetPercentFtp: 55,
            zoneId: 1,
          },
        ],
        targetPercentFtp: 105,
        zoneId: 4,
      },
      {
        id: 'section-2',
        label: 'Main Set 2 10x',
        repeatCount: 10,
        durationSeconds: 600,
        steps: [
          {
            id: 'section-step-3',
            label: '30s 130% FTP',
            detail: '30s • 130% FTP',
            durationSeconds: 30,
            targetPercentFtp: 130,
            zoneId: 6,
          },
          {
            id: 'section-step-4',
            label: '30s 50% FTP',
            detail: '30s • 50% FTP',
            durationSeconds: 30,
            targetPercentFtp: 50,
            zoneId: 1,
          },
        ],
        targetPercentFtp: 130,
        zoneId: 6,
      },
      {
        id: 'section-3',
        label: 'Cooldown',
        repeatCount: 1,
        durationSeconds: 600,
        steps: [
          {
            id: 'section-step-5',
            label: '10m 50% FTP',
            detail: '10m • 50% FTP',
            durationSeconds: 600,
            targetPercentFtp: 50,
            zoneId: 1,
          },
        ],
        targetPercentFtp: 50,
        zoneId: 1,
      },
    ]);
    expect(buildPlannedWorkoutBars(event).map((bar) => bar.widthUnits)).toEqual([
      1200,
      300,
      180,
      300,
      180,
      300,
      180,
      300,
      180,
      30,
      30,
      30,
      30,
      30,
      30,
      30,
      30,
      30,
      30,
      30,
      30,
      30,
      30,
      30,
      30,
      30,
      30,
      30,
      30,
      600,
    ]);
    expect(buildPlannedWorkoutChartIntervals(event).map((interval) => interval.label)).toEqual([
      '20m ramp 50-75% FTP',
      '5m 105% FTP 1/4',
      '3m 55% FTP 1/4',
      '5m 105% FTP 2/4',
      '3m 55% FTP 2/4',
      '5m 105% FTP 3/4',
      '3m 55% FTP 3/4',
      '5m 105% FTP 4/4',
      '3m 55% FTP 4/4',
      '30s 130% FTP 1/10',
      '30s 50% FTP 1/10',
      '30s 130% FTP 2/10',
      '30s 50% FTP 2/10',
      '30s 130% FTP 3/10',
      '30s 50% FTP 3/10',
      '30s 130% FTP 4/10',
      '30s 50% FTP 4/10',
      '30s 130% FTP 5/10',
      '30s 50% FTP 5/10',
      '30s 130% FTP 6/10',
      '30s 50% FTP 6/10',
      '30s 130% FTP 7/10',
      '30s 50% FTP 7/10',
      '30s 130% FTP 8/10',
      '30s 50% FTP 8/10',
      '30s 130% FTP 9/10',
      '30s 50% FTP 9/10',
      '30s 130% FTP 10/10',
      '30s 50% FTP 10/10',
      '10m 50% FTP',
    ]);
  });

  it('builds planned power series from segments for the detail chart', () => {
    const event: IntervalEvent = {
      id: 101,
      startDateLocal: '2026-03-30',
      name: 'Threshold Build',
      category: 'WORKOUT',
      description: null,
      restDay: false,
      restDayReason: null,
      indoor: false,
      color: null,
      eventDefinition: {
        rawWorkoutDoc: null,
        intervals: [],
        segments: [
          {
            order: 0,
            label: 'Warmup',
            durationSeconds: 600,
            startOffsetSeconds: 0,
            endOffsetSeconds: 600,
            targetPercentFtp: 65,
            zoneId: 2,
          },
          {
            order: 1,
            label: 'Threshold',
            durationSeconds: 300,
            startOffsetSeconds: 600,
            endOffsetSeconds: 900,
            targetPercentFtp: 100,
            zoneId: 4,
          },
        ],
        summary: {
          totalSegments: 2,
          totalDurationSeconds: 900,
          estimatedNormalizedPowerWatts: null,
          estimatedAveragePowerWatts: null,
          estimatedIntensityFactor: null,
          estimatedTrainingStressScore: null,
        },
      },
      actualWorkout: null,
    };

    expect(buildPlannedWorkoutPowerSeries(event, 300)).toEqual([65, 65, 100]);
  });

  it('falls back when raw workout section steps do not match parsed intervals', () => {
    const event: IntervalEvent = {
      id: 404,
      startDateLocal: '2026-04-10',
      name: 'Mismatch Workout',
      category: 'WORKOUT',
      description: null,
      restDay: false,
      restDayReason: null,
      indoor: true,
      color: null,
      eventDefinition: {
        rawWorkoutDoc: 'Mismatch Workout\nWarmup\n- 10m 55%\nMain Set\n- 5m 105%\n- 3m 55%',
        intervals: [
          {
            definition: '- 10m 55%',
            repeatCount: 1,
            durationSeconds: 600,
            targetPercentFtp: 55,
            zoneId: 1,
          },
        ],
        segments: [],
        summary: {
          totalSegments: 0,
          totalDurationSeconds: 600,
          estimatedNormalizedPowerWatts: null,
          estimatedAveragePowerWatts: null,
          estimatedIntensityFactor: null,
          estimatedTrainingStressScore: null,
        },
      },
      actualWorkout: null,
    };

    expect(buildPlannedWorkoutStructureSections(event)).toEqual([
      {
        id: 'interval-0',
        label: '10m 55% FTP',
        durationSeconds: 600,
        steps: [
          {
            id: 'interval-0-detail',
            label: '10m 55% FTP',
            detail: '10m • 55% FTP',
            durationSeconds: 600,
          },
        ],
      },
    ]);
  });

  it('expands repeated interval definitions in planned power series and chart intervals', () => {
    const event: IntervalEvent = {
      id: 102,
      startDateLocal: '2026-03-30',
      name: 'Repeat Build',
      category: 'WORKOUT',
      description: null,
      restDay: false,
      restDayReason: null,
      indoor: false,
      color: null,
      eventDefinition: {
        rawWorkoutDoc: '- 2x20min 90%',
        intervals: [
          {
            definition: '- 2x20min 90%',
            repeatCount: 2,
            durationSeconds: 1200,
            targetPercentFtp: 90,
            zoneId: 3,
          },
        ],
        segments: [],
        summary: {
          totalSegments: 1,
          totalDurationSeconds: 2400,
          estimatedNormalizedPowerWatts: null,
          estimatedAveragePowerWatts: null,
          estimatedIntensityFactor: null,
          estimatedTrainingStressScore: null,
        },
      },
      actualWorkout: null,
    };

    expect(buildPlannedWorkoutPowerSeries(event, 600)).toEqual([90, 90, 90, 90]);
    expect(buildPlannedWorkoutChartIntervals(event)).toEqual([
      {
        id: 'planned-102-interval-0',
        startSecond: 0,
        endSecond: 2400,
        label: '2 x 20min 90% FTP',
      },
    ]);
  });

  it('expands segment templates to the full planned duration when summary implies repeats', () => {
    const event: IntervalEvent = {
      id: 104,
      startDateLocal: '2026-03-30',
      name: 'VO2 Template',
      category: 'WORKOUT',
      description: null,
      restDay: false,
      restDayReason: null,
      indoor: false,
      color: null,
      eventDefinition: {
        rawWorkoutDoc: '- 4x120% ftp 2min and 2min of rest 50%',
        intervals: [
          {
            definition: '- 4x120% ftp 2min and 2min of rest 50%',
            repeatCount: 4,
            durationSeconds: 240,
            targetPercentFtp: 120,
            zoneId: 5,
          },
        ],
        segments: [
          {
            order: 0,
            label: 'Work',
            durationSeconds: 120,
            startOffsetSeconds: 0,
            endOffsetSeconds: 120,
            targetPercentFtp: 120,
            zoneId: 5,
          },
          {
            order: 1,
            label: 'Rest',
            durationSeconds: 120,
            startOffsetSeconds: 120,
            endOffsetSeconds: 240,
            targetPercentFtp: 50,
            zoneId: 1,
          },
        ],
        summary: {
          totalSegments: 2,
          totalDurationSeconds: 960,
          estimatedNormalizedPowerWatts: null,
          estimatedAveragePowerWatts: null,
          estimatedIntensityFactor: null,
          estimatedTrainingStressScore: null,
        },
      },
      actualWorkout: null,
    };

    expect(buildPlannedWorkoutBars(event).map((bar) => bar.widthUnits)).toEqual([120, 120, 120, 120, 120, 120, 120, 120]);
    expect(buildPlannedWorkoutPowerSeries(event, 120)).toEqual([120, 50, 120, 50, 120, 50, 120, 50]);
    expect(buildPlannedWorkoutChartIntervals(event)).toHaveLength(8);
    expect(buildPlannedWorkoutChartIntervals(event).at(-1)?.endSecond).toBe(960);
  });

  it('does not treat non-workout events as planned workouts', () => {
    const event: IntervalEvent = {
      id: 103,
      startDateLocal: '2026-03-30',
      name: 'A race',
      category: 'RACE',
      description: null,
      restDay: false,
      restDayReason: null,
      indoor: false,
      color: null,
      eventDefinition: {
        rawWorkoutDoc: '- 20min 90%',
        intervals: [],
        segments: [],
        summary: {
          totalSegments: 0,
          totalDurationSeconds: 1200,
          estimatedNormalizedPowerWatts: null,
          estimatedAveragePowerWatts: null,
          estimatedIntensityFactor: null,
          estimatedTrainingStressScore: null,
        },
      },
      actualWorkout: null,
    };

    expect(isPlannedWorkoutEvent(event)).toBe(false);
  });

  it('formats compact duration labels', () => {
    expect(formatDurationLabel(45)).toBe('45s');
    expect(formatDurationLabel(3900)).toBe('1h 05m');
    expect(formatDurationLabel(1500)).toBe('25m');
    expect(formatDurationLabel(3599)).toBe('59m');
    expect(formatDurationLabel(7199)).toBe('1h 59m');
  });

  it('builds a 5 second average power series', () => {
    expect(buildFiveSecondAveragePowerSeries([100, 150, 200, 250, 300, 350, 400])).toEqual([100, 125, 150, 175, 200, 250, 300]);
  });

  it('selects the matched activity from the full day activity list', () => {
    const event: IntervalEvent = {
      id: 10,
      startDateLocal: '2026-03-22',
      name: 'Workout',
      category: 'WORKOUT',
      description: null,
      restDay: false,
      restDayReason: null,
      indoor: false,
      color: null,
      eventDefinition: {
        rawWorkoutDoc: null,
        intervals: [],
        segments: [],
        summary: {
          totalSegments: 0,
          totalDurationSeconds: 0,
          estimatedNormalizedPowerWatts: null,
          estimatedAveragePowerWatts: null,
          estimatedIntensityFactor: null,
          estimatedTrainingStressScore: null,
        },
      },
      actualWorkout: {
        activityId: 'a-match',
        activityName: 'Matched ride',
        startDateLocal: '2026-03-22T08:00:00',
        powerValues: [],
        cadenceValues: [],
        heartRateValues: [],
        speedValues: [],
        averagePowerWatts: null,
        normalizedPowerWatts: null,
        trainingStressScore: null,
        intensityFactor: null,
        complianceScore: 0.8,
        matchedIntervals: [],
      },
    };

    const activityBase: IntervalActivity = {
      id: 'a-other',
      startDateLocal: '2026-03-22T07:00:00',
      startDate: '2026-03-22T06:00:00Z',
      name: 'Other ride',
      description: null,
      activityType: 'Ride',
      source: null,
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
        strainScore: null,
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
        gapZoneTimes: [],
      },
      detailsUnavailableReason: null,
    };

    const selection = selectWorkoutDetail('2026-03-22', event, [activityBase, { ...activityBase, id: 'a-match', name: 'Matched ride' }]);

    expect(selection.activity?.id).toBe('a-match');
    expect(selection.event?.id).toBe(event.id);
  });

  it('keeps an unrelated non-planned event selection in completed mode instead of forcing the event', () => {
    const event: IntervalEvent = {
      id: 14,
      startDateLocal: '2026-03-22',
      name: 'Planned workout',
      category: 'WORKOUT',
      description: null,
      restDay: false,
      restDayReason: null,
      indoor: false,
      color: null,
      eventDefinition: {
        rawWorkoutDoc: null,
        intervals: [],
        segments: [],
        summary: {
          totalSegments: 0,
          totalDurationSeconds: 0,
          estimatedNormalizedPowerWatts: null,
          estimatedAveragePowerWatts: null,
          estimatedIntensityFactor: null,
          estimatedTrainingStressScore: null,
        },
      },
      actualWorkout: null,
    };

    const activity: IntervalActivity = {
      id: 'a-unrelated',
      startDateLocal: '2026-03-22T08:00:00',
      startDate: '2026-03-22T07:00:00Z',
      name: 'Unrelated ride',
      description: null,
      activityType: 'Ride',
      source: null,
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
        strainScore: null,
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
        gapZoneTimes: [],
      },
      detailsUnavailableReason: null,
    };

    const selection = selectWorkoutDetail('2026-03-22', event, [activity]);

    expect(selection).toEqual({
      dateKey: '2026-03-22',
      event: null,
      activity,
    });
  });

  it('keeps a real planned workout alongside an unrelated same-day activity', () => {
    const event: IntervalEvent = {
      id: 16,
      startDateLocal: '2026-03-22',
      name: 'Planned workout',
      category: 'WORKOUT',
      description: null,
      restDay: false,
      restDayReason: null,
      indoor: false,
      color: null,
      eventDefinition: {
        rawWorkoutDoc: '- 2x20min 90%',
        intervals: [
          {
            definition: '- 2x20min 90%',
            repeatCount: 2,
            durationSeconds: 1200,
            targetPercentFtp: 90,
            zoneId: 3,
          },
        ],
        segments: [],
        summary: {
          totalSegments: 1,
          totalDurationSeconds: 2400,
          estimatedNormalizedPowerWatts: null,
          estimatedAveragePowerWatts: null,
          estimatedIntensityFactor: null,
          estimatedTrainingStressScore: null,
        },
      },
      actualWorkout: null,
    };

    const activity: IntervalActivity = {
      id: 'a-unrelated',
      startDateLocal: '2026-03-22T08:00:00',
      startDate: '2026-03-22T07:00:00Z',
      name: 'Unrelated ride',
      description: null,
      activityType: 'Ride',
      source: null,
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
        strainScore: null,
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
        gapZoneTimes: [],
      },
      detailsUnavailableReason: null,
    };

    expect(selectWorkoutDetail('2026-03-22', event, [activity])).toEqual({
      dateKey: '2026-03-22',
      event,
      activity,
    });
  });

  it('keeps completed events selectable when activities are not loaded yet', () => {
    const event: IntervalEvent = {
      id: 15,
      startDateLocal: '2026-03-22',
      name: 'Completed workout',
      category: 'WORKOUT',
      description: null,
      restDay: false,
      restDayReason: null,
      indoor: false,
      color: null,
      eventDefinition: {
        rawWorkoutDoc: null,
        intervals: [],
        segments: [],
        summary: {
          totalSegments: 0,
          totalDurationSeconds: 0,
          estimatedNormalizedPowerWatts: null,
          estimatedAveragePowerWatts: null,
          estimatedIntensityFactor: null,
          estimatedTrainingStressScore: null,
        },
      },
      actualWorkout: {
        activityId: 'a-completed',
        activityName: 'Completed workout outside',
        startDateLocal: '2026-03-22T08:00:00',
        powerValues: [],
        cadenceValues: [],
        heartRateValues: [],
        speedValues: [],
        averagePowerWatts: null,
        normalizedPowerWatts: null,
        trainingStressScore: null,
        intensityFactor: null,
        complianceScore: 0.8,
        matchedIntervals: [],
      },
    };

    expect(selectWorkoutDetail('2026-03-22', event, [])).toEqual({
      dateKey: '2026-03-22',
      event,
      activity: null,
    });
  });

  it('does not pair a completed event with an unrelated visible activity when the match is missing', () => {
    const event: IntervalEvent = {
      id: 19,
      startDateLocal: '2026-03-22',
      name: 'Completed workout',
      category: 'WORKOUT',
      description: null,
      restDay: false,
      restDayReason: null,
      indoor: false,
      color: null,
      eventDefinition: {
        rawWorkoutDoc: '- 20min 95%',
        intervals: [],
        segments: [],
        summary: {
          totalSegments: 0,
          totalDurationSeconds: 0,
          estimatedNormalizedPowerWatts: null,
          estimatedAveragePowerWatts: null,
          estimatedIntensityFactor: null,
          estimatedTrainingStressScore: null,
        },
      },
      actualWorkout: {
        activityId: 'a-match',
        activityName: 'Matched ride',
        startDateLocal: '2026-03-22T08:00:00',
        powerValues: [],
        cadenceValues: [],
        heartRateValues: [],
        speedValues: [],
        averagePowerWatts: null,
        normalizedPowerWatts: null,
        trainingStressScore: null,
        intensityFactor: null,
        complianceScore: 0.8,
        matchedIntervals: [],
      },
    };

    const activity: IntervalActivity = {
      id: 'a-unrelated',
      startDateLocal: '2026-03-22T07:00:00',
      startDate: '2026-03-22T06:00:00Z',
      name: 'Morning spin',
      description: null,
      activityType: 'Ride',
      source: null,
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
        strainScore: null,
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
        gapZoneTimes: [],
      },
      detailsUnavailableReason: null,
    };

    expect(selectWorkoutDetail('2026-03-22', event, [activity])).toEqual({
      dateKey: '2026-03-22',
      event: null,
      activity,
    });
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
