import { cleanup, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it, vi } from 'vitest';

import '../../../i18n';
import { downloadFit, loadActivity, loadEvent } from '../../intervals/api/intervals';
import type { WorkoutDetailSelection } from '../workoutDetails';
import { WorkoutDetailModal } from './WorkoutDetailModal';

vi.mock('../../intervals/api/intervals', () => ({
  downloadFit: vi.fn(),
  loadEvent: vi.fn(),
  loadActivity: vi.fn(),
}));

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

describe('WorkoutDetailModal', () => {
  function metricCard(label: string) {
    return screen.getByText(label).closest('div');
  }

  it('loads and renders a planned workout detail view', async () => {
    vi.mocked(loadEvent).mockResolvedValue({
      id: 11,
      startDateLocal: '2026-03-25',
      name: 'Sweet Spot',
      category: 'WORKOUT',
      description: null,
      indoor: true,
      color: null,
      eventDefinition: {
        rawWorkoutDoc: '- 3x8min 95%',
        intervals: [{ definition: '- 3x8min 95%', repeatCount: 3, durationSeconds: 480, targetPercentFtp: 95, zoneId: 4 }],
        segments: Array.from({ length: 3 }, (_, index) => ({
          order: index,
          label: `3x8min 95% #${index + 1}`,
          durationSeconds: 480,
          startOffsetSeconds: index * 480,
          endOffsetSeconds: (index + 1) * 480,
          targetPercentFtp: 95,
          zoneId: 4,
        })),
        summary: {
          totalSegments: 3,
          totalDurationSeconds: 1440,
          estimatedNormalizedPowerWatts: 285,
          estimatedAveragePowerWatts: 278,
          estimatedIntensityFactor: 0.95,
          estimatedTrainingStressScore: 38,
        },
      },
      actualWorkout: null,
    });
    vi.mocked(loadActivity).mockResolvedValue(undefined as never);

    const selection: WorkoutDetailSelection = {
      dateKey: '2026-03-25',
      event: {
        id: 11,
        startDateLocal: '2026-03-25',
        name: 'Sweet Spot',
        category: 'WORKOUT',
        description: null,
        indoor: true,
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
      },
      activity: null,
    };

    render(<WorkoutDetailModal apiBaseUrl="" selection={selection} onClose={vi.fn()} />);

    await waitFor(() => {
      expect(screen.getByText('Sweet Spot')).toBeInTheDocument();
    });

    expect(screen.getByText(/planned workout/i)).toBeInTheDocument();
    expect(screen.getByText(/24m/i)).toBeInTheDocument();
    expect(screen.getByText(/0.95 IF/i)).toBeInTheDocument();
  });

  it('loads and renders a completed workout detail view with comparison data', async () => {
    vi.mocked(loadEvent).mockResolvedValue({
      id: 21,
      startDateLocal: '2026-03-25',
      name: 'Threshold',
      category: 'WORKOUT',
      description: null,
      indoor: false,
      color: null,
      eventDefinition: {
        rawWorkoutDoc: '- 20min 95%',
        intervals: [{ definition: '- 20min 95%', repeatCount: 1, durationSeconds: 1200, targetPercentFtp: 95, zoneId: 4 }],
        segments: [{ order: 0, label: '20min 95%', durationSeconds: 1200, startOffsetSeconds: 0, endOffsetSeconds: 1200, targetPercentFtp: 95, zoneId: 4 }],
        summary: {
          totalSegments: 1,
          totalDurationSeconds: 1200,
          estimatedNormalizedPowerWatts: 285,
          estimatedAveragePowerWatts: 285,
          estimatedIntensityFactor: 0.95,
          estimatedTrainingStressScore: 30.1,
        },
      },
      actualWorkout: {
        activityId: 'a21',
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
        matchedIntervals: [{
          plannedSegmentOrder: 0,
          plannedLabel: '20min 95%',
          plannedDurationSeconds: 1200,
          targetPercentFtp: 95,
          zoneId: 4,
          actualIntervalId: 1,
          actualStartTimeSeconds: 600,
          actualEndTimeSeconds: 1800,
          averagePowerWatts: 271,
          normalizedPowerWatts: 280,
          averageHeartRateBpm: 161,
          averageCadenceRpm: 89,
          averageSpeedMps: 10.1,
          complianceScore: 0.91,
        }],
      },
    });
    vi.mocked(loadActivity).mockResolvedValue({
      id: 'a21',
      startDateLocal: '2026-03-25T08:00:00',
      startDate: '2026-03-25T07:00:00Z',
      name: 'Threshold Ride',
      description: null,
      activityType: 'Ride',
      source: null,
      externalId: null,
      deviceName: null,
      distanceMeters: 40000,
      movingTimeSeconds: 3600,
      elapsedTimeSeconds: 3650,
      totalElevationGainMeters: null,
      averageSpeedMps: 10.1,
      averageHeartRateBpm: 158,
      averageCadenceRpm: 89,
      trainer: false,
      commute: false,
      race: false,
      hasHeartRate: true,
      streamTypes: ['watts', 'heartrate', 'cadence'],
      tags: [],
      metrics: {
        trainingStressScore: 78,
        normalizedPowerWatts: 280,
        intensityFactor: 0.93,
        efficiencyFactor: null,
        variabilityIndex: null,
        averagePowerWatts: 271,
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
        intervals: [],
        intervalGroups: [],
        streams: [
          { streamType: 'watts', name: 'Power', data: [150, 210, 290, 275], data2: null, valueTypeIsArray: false, custom: false, allNull: false },
          { streamType: 'heartrate', name: 'Heart Rate', data: [130, 145, 162, 165], data2: null, valueTypeIsArray: false, custom: false, allNull: false },
          { streamType: 'cadence', name: 'Cadence', data: [85, 88, 92, 89], data2: null, valueTypeIsArray: false, custom: false, allNull: false },
        ],
        intervalSummary: [],
        skylineChart: [],
        powerZoneTimes: [],
        heartRateZoneTimes: [],
        paceZoneTimes: [],
        gapZoneTimes: [],
      },
    });

    const selection: WorkoutDetailSelection = {
      dateKey: '2026-03-25',
      event: {
        id: 21,
        startDateLocal: '2026-03-25',
        name: 'Threshold',
        category: 'WORKOUT',
        description: null,
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
      },
      activity: {
        id: 'a21',
        startDateLocal: '2026-03-25T08:00:00',
        startDate: '2026-03-25T07:00:00Z',
        name: 'Threshold Ride',
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
        hasHeartRate: true,
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
      },
    };

    const onClose = vi.fn();
    render(<WorkoutDetailModal apiBaseUrl="" selection={selection} onClose={onClose} />);

    await waitFor(() => {
      expect(screen.getByText(/completed workout/i)).toBeInTheDocument();
    });

    expect(screen.getByText('Threshold Ride')).toBeInTheDocument();
    expect(within(metricCard('Duration') as HTMLElement).getByText('20m')).toBeInTheDocument();
    expect(within(metricCard('NP') as HTMLElement).getByText('280 W')).toBeInTheDocument();
    expect(within(metricCard('TSS') as HTMLElement).getByText('35 TSS')).toBeInTheDocument();
    expect(screen.queryByText('78 TSS')).not.toBeInTheDocument();
    expect(screen.getAllByText(/91% compliance/i)).toHaveLength(2);
    expect(document.querySelectorAll('.rounded-t-md')).toHaveLength(4);

    await userEvent.click(screen.getByRole('button', { name: /close workout details/i }));
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it('keeps planned workout details visible when activity loading fails', async () => {
    vi.mocked(loadEvent).mockResolvedValue({
      id: 12,
      startDateLocal: '2026-03-25',
      name: 'Tempo Builder',
      category: 'WORKOUT',
      description: null,
      indoor: true,
      color: null,
      eventDefinition: {
        rawWorkoutDoc: '- 40min 80%',
        intervals: [{ definition: '- 40min 80%', repeatCount: 1, durationSeconds: 2400, targetPercentFtp: 80, zoneId: 3 }],
        segments: [{ order: 0, label: '40min 80%', durationSeconds: 2400, startOffsetSeconds: 0, endOffsetSeconds: 2400, targetPercentFtp: 80, zoneId: 3 }],
        summary: {
          totalSegments: 1,
          totalDurationSeconds: 2400,
          estimatedNormalizedPowerWatts: 240,
          estimatedAveragePowerWatts: 235,
          estimatedIntensityFactor: 0.8,
          estimatedTrainingStressScore: 42,
        },
      },
      actualWorkout: null,
    });
    vi.mocked(loadActivity).mockRejectedValue(new Error('activity fetch failed'));

    const selection: WorkoutDetailSelection = {
      dateKey: '2026-03-25',
      event: {
        id: 12,
        startDateLocal: '2026-03-25',
        name: 'Tempo Builder',
        category: 'WORKOUT',
        description: null,
        indoor: true,
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
      },
      activity: {
        id: 'missing-activity',
        startDateLocal: '2026-03-25T08:00:00',
        startDate: '2026-03-25T07:00:00Z',
        name: 'Missing Activity',
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
      },
    };

    render(<WorkoutDetailModal apiBaseUrl="" selection={selection} onClose={vi.fn()} />);

    await waitFor(() => {
      expect(screen.getByText('Tempo Builder')).toBeInTheDocument();
    });

    expect(screen.getByText(/planned workout/i)).toBeInTheDocument();
    expect(within(metricCard('Duration') as HTMLElement).getByText('40m')).toBeInTheDocument();
    expect(screen.getByText(/0.80 IF/i)).toBeInTheDocument();
    expect(within(metricCard('TSS') as HTMLElement).getByText('42 TSS')).toBeInTheDocument();
    expect(within(metricCard('NP') as HTMLElement).getByText('240 W')).toBeInTheDocument();
    expect(screen.queryByText('activity fetch failed')).not.toBeInTheDocument();
    expect(document.querySelectorAll('.rounded-t-md')).toHaveLength(1);
  });

  it('keeps selected activity details visible when activity reload fails for an activity-only day', async () => {
    vi.mocked(loadEvent).mockResolvedValue(undefined as never);
    vi.mocked(loadActivity).mockRejectedValue(new Error('activity fetch failed'));

    const selection: WorkoutDetailSelection = {
      dateKey: '2026-03-27',
      event: null,
      activity: {
        id: 'a24',
        startDateLocal: '2026-03-27T08:00:00',
        startDate: '2026-03-27T07:00:00Z',
        name: 'Solo ride',
        description: null,
        activityType: 'Ride',
        source: null,
        externalId: null,
        deviceName: null,
        distanceMeters: null,
        movingTimeSeconds: 2700,
        elapsedTimeSeconds: 2750,
        totalElevationGainMeters: null,
        averageSpeedMps: null,
        averageHeartRateBpm: null,
        averageCadenceRpm: null,
        trainer: false,
        commute: false,
        race: false,
        hasHeartRate: false,
        streamTypes: ['watts'],
        tags: [],
        metrics: {
          trainingStressScore: 62,
          normalizedPowerWatts: 228,
          intensityFactor: 0.81,
          efficiencyFactor: null,
          variabilityIndex: null,
          averagePowerWatts: 219,
          ftpWatts: 280,
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
            { streamType: 'watts', name: 'Power', data: [180, 220, 260], data2: null, valueTypeIsArray: false, custom: false, allNull: false },
          ],
          intervalSummary: [],
          skylineChart: [],
          powerZoneTimes: [],
          heartRateZoneTimes: [],
          paceZoneTimes: [],
          gapZoneTimes: [],
        },
      },
    };

    render(<WorkoutDetailModal apiBaseUrl="" selection={selection} onClose={vi.fn()} />);

    await waitFor(() => {
      expect(screen.getByText(/completed workout/i)).toBeInTheDocument();
    });

    expect(screen.getByText('Solo ride')).toBeInTheDocument();
    expect(screen.getByText('228 W')).toBeInTheDocument();
    expect(screen.getByText('62 TSS')).toBeInTheDocument();
  });

  it('renders completed-only interval sections from enriched activity details', async () => {
    vi.mocked(loadEvent).mockResolvedValue(undefined as never);
    vi.mocked(loadActivity).mockResolvedValue({
      id: 'a25',
      startDateLocal: '2026-03-28T08:00:00',
      startDate: '2026-03-28T07:00:00Z',
      name: 'Outside Tempo',
      description: null,
      activityType: 'Ride',
      source: null,
      externalId: null,
      deviceName: null,
      distanceMeters: 32000,
      movingTimeSeconds: 3600,
      elapsedTimeSeconds: 3660,
      totalElevationGainMeters: null,
      averageSpeedMps: 8.9,
      averageHeartRateBpm: 151,
      averageCadenceRpm: 88,
      trainer: false,
      commute: false,
      race: false,
      hasHeartRate: true,
      streamTypes: ['watts', 'heartrate'],
      tags: [],
      metrics: {
        trainingStressScore: 74,
        normalizedPowerWatts: 249,
        intensityFactor: 0.89,
        efficiencyFactor: null,
        variabilityIndex: null,
        averagePowerWatts: 236,
        ftpWatts: 280,
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
            label: 'Tempo Block 1',
            intervalType: 'WORK',
            groupId: 'tempo',
            startIndex: 0,
            endIndex: 599,
            startTimeSeconds: 0,
            endTimeSeconds: 600,
            movingTimeSeconds: 600,
            elapsedTimeSeconds: 600,
            distanceMeters: 5000,
            averagePowerWatts: 245,
            normalizedPowerWatts: 252,
            trainingStressScore: 12.4,
            averageHeartRateBpm: 148,
            averageCadenceRpm: 87,
            averageSpeedMps: 8.3,
            averageStrideMeters: null,
            zone: 3,
          },
          {
            id: 2,
            label: 'Tempo Block 2',
            intervalType: 'WORK',
            groupId: 'tempo',
            startIndex: 900,
            endIndex: 1499,
            startTimeSeconds: 900,
            endTimeSeconds: 1500,
            movingTimeSeconds: 600,
            elapsedTimeSeconds: 600,
            distanceMeters: 5100,
            averagePowerWatts: 255,
            normalizedPowerWatts: 261,
            trainingStressScore: 13.2,
            averageHeartRateBpm: 154,
            averageCadenceRpm: 89,
            averageSpeedMps: 8.5,
            averageStrideMeters: null,
            zone: 4,
          },
        ],
        intervalGroups: [],
        streams: [
          { streamType: 'watts', name: 'Power', data: [210, 240, 255, 260], data2: null, valueTypeIsArray: false, custom: false, allNull: false },
          { streamType: 'heartrate', name: 'Heart Rate', data: [138, 146, 152, 156], data2: null, valueTypeIsArray: false, custom: false, allNull: false },
        ],
        intervalSummary: [],
        skylineChart: [],
        powerZoneTimes: [],
        heartRateZoneTimes: [],
        paceZoneTimes: [],
        gapZoneTimes: [],
      },
    });

    const selection: WorkoutDetailSelection = {
      dateKey: '2026-03-28',
      event: null,
      activity: {
        id: 'a25',
        startDateLocal: '2026-03-28T08:00:00',
        startDate: '2026-03-28T07:00:00Z',
        name: 'Outside Tempo',
        description: null,
        activityType: 'Ride',
        source: null,
        externalId: null,
        deviceName: null,
        distanceMeters: null,
        movingTimeSeconds: 3600,
        elapsedTimeSeconds: 3660,
        totalElevationGainMeters: null,
        averageSpeedMps: null,
        averageHeartRateBpm: null,
        averageCadenceRpm: null,
        trainer: false,
        commute: false,
        race: false,
        hasHeartRate: true,
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
      },
    };

    render(<WorkoutDetailModal apiBaseUrl="" selection={selection} onClose={vi.fn()} />);

    await waitFor(() => {
      expect(screen.getByText(/completed workout/i)).toBeInTheDocument();
    });

    expect(screen.getByText('Outside Tempo')).toBeInTheDocument();
    expect(screen.getByText('249 W')).toBeInTheDocument();
    expect(screen.getByText('74 TSS')).toBeInTheDocument();
    expect(screen.getByText(/completed intervals/i)).toBeInTheDocument();
    expect(screen.getByText('Tempo Block 1')).toBeInTheDocument();
    expect(screen.getByText('Tempo Block 2')).toBeInTheDocument();
    expect(screen.getByText('245 W')).toBeInTheDocument();
    expect(screen.getByText('255 W')).toBeInTheDocument();
    expect(screen.getAllByText('10m')).toHaveLength(2);
  });

  it('excludes metadata-only completed intervals from the rendered section', async () => {
    vi.mocked(loadEvent).mockResolvedValue(undefined as never);
    vi.mocked(loadActivity).mockResolvedValue({
      id: 'a26',
      startDateLocal: '2026-03-29T08:00:00',
      startDate: '2026-03-29T07:00:00Z',
      name: 'Metadata Filter Ride',
      description: null,
      activityType: 'Ride',
      source: null,
      externalId: null,
      deviceName: null,
      distanceMeters: 28000,
      movingTimeSeconds: 2400,
      elapsedTimeSeconds: 2460,
      totalElevationGainMeters: null,
      averageSpeedMps: 8.2,
      averageHeartRateBpm: 149,
      averageCadenceRpm: 87,
      trainer: false,
      commute: false,
      race: false,
      hasHeartRate: true,
      streamTypes: ['watts'],
      tags: [],
      metrics: {
        trainingStressScore: 46,
        normalizedPowerWatts: 226,
        intensityFactor: 0.81,
        efficiencyFactor: null,
        variabilityIndex: null,
        averagePowerWatts: 214,
        ftpWatts: 280,
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
            id: 91,
            label: null,
            intervalType: 'WORK',
            groupId: 'meta',
            startIndex: 0,
            endIndex: 599,
            startTimeSeconds: 0,
            endTimeSeconds: 600,
            movingTimeSeconds: null,
            elapsedTimeSeconds: null,
            distanceMeters: 4200,
            averagePowerWatts: null,
            normalizedPowerWatts: 230,
            trainingStressScore: 9.2,
            averageHeartRateBpm: null,
            averageCadenceRpm: 88,
            averageSpeedMps: 8.1,
            averageStrideMeters: null,
            zone: 3,
          },
          {
            id: 92,
            label: 'Shown Interval',
            intervalType: 'WORK',
            groupId: 'meta',
            startIndex: 600,
            endIndex: 1199,
            startTimeSeconds: 600,
            endTimeSeconds: 1200,
            movingTimeSeconds: null,
            elapsedTimeSeconds: 600,
            distanceMeters: null,
            averagePowerWatts: null,
            normalizedPowerWatts: null,
            trainingStressScore: null,
            averageHeartRateBpm: null,
            averageCadenceRpm: null,
            averageSpeedMps: null,
            averageStrideMeters: null,
            zone: null,
          },
        ],
        intervalGroups: [],
        streams: [
          { streamType: 'watts', name: 'Power', data: [180, 220, 230], data2: null, valueTypeIsArray: false, custom: false, allNull: false },
        ],
        intervalSummary: [],
        skylineChart: [],
        powerZoneTimes: [],
        heartRateZoneTimes: [],
        paceZoneTimes: [],
        gapZoneTimes: [],
      },
    });

    const selection: WorkoutDetailSelection = {
      dateKey: '2026-03-29',
      event: null,
      activity: {
        id: 'a26',
        startDateLocal: '2026-03-29T08:00:00',
        startDate: '2026-03-29T07:00:00Z',
        name: 'Metadata Filter Ride',
        description: null,
        activityType: 'Ride',
        source: null,
        externalId: null,
        deviceName: null,
        distanceMeters: null,
        movingTimeSeconds: 2400,
        elapsedTimeSeconds: 2460,
        totalElevationGainMeters: null,
        averageSpeedMps: null,
        averageHeartRateBpm: null,
        averageCadenceRpm: null,
        trainer: false,
        commute: false,
        race: false,
        hasHeartRate: true,
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
      },
    };

    render(<WorkoutDetailModal apiBaseUrl="" selection={selection} onClose={vi.fn()} />);

    await waitFor(() => {
      expect(screen.getByText(/completed workout/i)).toBeInTheDocument();
    });

    expect(screen.getByText(/completed intervals/i)).toBeInTheDocument();
    expect(screen.getByText('Shown Interval')).toBeInTheDocument();
    expect(screen.queryByText('Ride 1')).not.toBeInTheDocument();
    expect(screen.getAllByText('10m')).toHaveLength(1);
  });

  it('renders enriched completed-only title duration metrics and chart bars from activity payloads', async () => {
    vi.mocked(loadEvent).mockResolvedValue(undefined as never);
    vi.mocked(loadActivity).mockResolvedValue({
      id: 'a27',
      startDateLocal: '2026-03-30T06:45:00',
      startDate: '2026-03-30T05:45:00Z',
      name: null,
      description: null,
      activityType: 'Ride',
      source: 'STRAVA',
      externalId: null,
      deviceName: null,
      distanceMeters: 42000,
      movingTimeSeconds: 0,
      elapsedTimeSeconds: 5520,
      totalElevationGainMeters: null,
      averageSpeedMps: 7.8,
      averageHeartRateBpm: 144,
      averageCadenceRpm: 86,
      trainer: false,
      commute: false,
      race: false,
      hasHeartRate: true,
      streamTypes: ['watts'],
      tags: [],
      metrics: {
        trainingStressScore: 67,
        normalizedPowerWatts: 238,
        intensityFactor: 0.79,
        efficiencyFactor: null,
        variabilityIndex: null,
        averagePowerWatts: 224,
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
            label: 'Steady Block',
            intervalType: 'WORK',
            groupId: 'steady',
            startIndex: 0,
            endIndex: 1199,
            startTimeSeconds: 0,
            endTimeSeconds: 1200,
            movingTimeSeconds: 1200,
            elapsedTimeSeconds: 1200,
            distanceMeters: 9000,
            averagePowerWatts: 228,
            normalizedPowerWatts: 236,
            trainingStressScore: 15.2,
            averageHeartRateBpm: 142,
            averageCadenceRpm: 85,
            averageSpeedMps: 7.6,
            averageStrideMeters: null,
            zone: 3,
          },
          {
            id: 2,
            label: 'Finish Block',
            intervalType: 'WORK',
            groupId: 'steady',
            startIndex: 1200,
            endIndex: 2399,
            startTimeSeconds: 1200,
            endTimeSeconds: 2400,
            movingTimeSeconds: 1200,
            elapsedTimeSeconds: 1200,
            distanceMeters: 9200,
            averagePowerWatts: 232,
            normalizedPowerWatts: 240,
            trainingStressScore: 15.6,
            averageHeartRateBpm: 145,
            averageCadenceRpm: 86,
            averageSpeedMps: 7.8,
            averageStrideMeters: null,
            zone: 3,
          },
        ],
        intervalGroups: [],
        streams: [
          { streamType: 'watts', name: 'Power', data: [180, 220, 245, 235, 250], data2: null, valueTypeIsArray: false, custom: false, allNull: false },
        ],
        intervalSummary: [],
        skylineChart: [],
        powerZoneTimes: [],
        heartRateZoneTimes: [],
        paceZoneTimes: [],
        gapZoneTimes: [],
      },
    });

    const selection: WorkoutDetailSelection = {
      dateKey: '2026-03-30',
      event: null,
      activity: {
        id: 'a27',
        startDateLocal: '2026-03-30T06:45:00',
        startDate: '2026-03-30T05:45:00Z',
        name: null,
        description: null,
        activityType: 'Ride',
        source: null,
        externalId: null,
        deviceName: null,
        distanceMeters: null,
        movingTimeSeconds: 0,
        elapsedTimeSeconds: 0,
        totalElevationGainMeters: null,
        averageSpeedMps: null,
        averageHeartRateBpm: null,
        averageCadenceRpm: null,
        trainer: false,
        commute: false,
        race: false,
        hasHeartRate: true,
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
      },
    };

    const { container } = render(<WorkoutDetailModal apiBaseUrl="" selection={selection} onClose={vi.fn()} />);

    await waitFor(() => {
      expect(screen.getByText(/completed workout/i)).toBeInTheDocument();
    });

    expect(screen.getByRole('heading', { name: 'Ride' })).toBeInTheDocument();
    expect(screen.queryByRole('heading', { name: 'Workout' })).not.toBeInTheDocument();
    expect(screen.getByText('1h 32m')).toBeInTheDocument();
    expect(screen.getByText('238 W')).toBeInTheDocument();
    expect(screen.getByText('67 TSS')).toBeInTheDocument();
    expect(container.querySelectorAll('.rounded-t-md')).toHaveLength(2);
  });

  it('stays in planned mode when an unrelated selected activity exists', async () => {
    vi.mocked(loadEvent).mockResolvedValue({
      id: 32,
      startDateLocal: '2026-03-27',
      name: 'Plan only',
      category: 'WORKOUT',
      description: null,
      indoor: true,
      color: null,
      eventDefinition: {
        rawWorkoutDoc: '- 30min 85%',
        intervals: [{ definition: '- 30min 85%', repeatCount: 1, durationSeconds: 1800, targetPercentFtp: 85, zoneId: 3 }],
        segments: [{ order: 0, label: '30min 85%', durationSeconds: 1800, startOffsetSeconds: 0, endOffsetSeconds: 1800, targetPercentFtp: 85, zoneId: 3 }],
        summary: {
          totalSegments: 1,
          totalDurationSeconds: 1800,
          estimatedNormalizedPowerWatts: 255,
          estimatedAveragePowerWatts: 255,
          estimatedIntensityFactor: 0.85,
          estimatedTrainingStressScore: 36.1,
        },
      },
      actualWorkout: null,
    });
    vi.mocked(loadActivity).mockResolvedValue({
      id: 'a-unrelated',
      startDateLocal: '2026-03-27T08:00:00',
      startDate: '2026-03-27T07:00:00Z',
      name: 'Unrelated ride',
      description: null,
      activityType: 'Ride',
      source: null,
      externalId: null,
      deviceName: null,
      distanceMeters: null,
      movingTimeSeconds: 2700,
      elapsedTimeSeconds: 2750,
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
        trainingStressScore: 62,
        normalizedPowerWatts: 228,
        intensityFactor: 0.81,
        efficiencyFactor: null,
        variabilityIndex: null,
        averagePowerWatts: 219,
        ftpWatts: 280,
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
    });

    const selection: WorkoutDetailSelection = {
      dateKey: '2026-03-27',
      event: {
        id: 32,
        startDateLocal: '2026-03-27',
        name: 'Plan only',
        category: 'WORKOUT',
        description: null,
        indoor: true,
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
      },
      activity: {
        id: 'a-unrelated',
        startDateLocal: '2026-03-27T08:00:00',
        startDate: '2026-03-27T07:00:00Z',
        name: 'Unrelated ride',
        description: null,
        activityType: 'Ride',
        source: null,
        externalId: null,
        deviceName: null,
        distanceMeters: null,
        movingTimeSeconds: 1200,
        elapsedTimeSeconds: 1260,
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
          trainingStressScore: 18,
          normalizedPowerWatts: 180,
          intensityFactor: 0.64,
          efficiencyFactor: null,
          variabilityIndex: null,
          averagePowerWatts: 172,
          ftpWatts: 280,
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
      },
    };

    render(<WorkoutDetailModal apiBaseUrl="" selection={selection} onClose={vi.fn()} />);

    await waitFor(() => {
      expect(screen.getByText('Plan only')).toBeInTheDocument();
    });

    expect(screen.getByText(/planned workout/i)).toBeInTheDocument();
    expect(screen.queryByText('Unrelated ride')).not.toBeInTheDocument();
    expect(within(metricCard('Duration') as HTMLElement).getByText('30m')).toBeInTheDocument();
    expect(within(metricCard('IF') as HTMLElement).getByText('0.85 IF')).toBeInTheDocument();
    expect(within(metricCard('TSS') as HTMLElement).getByText('36 TSS')).toBeInTheDocument();
    expect(within(metricCard('NP') as HTMLElement).getByText('255 W')).toBeInTheDocument();
    expect(screen.queryByText('18 TSS')).not.toBeInTheDocument();
    expect(screen.queryByText('228 W')).not.toBeInTheDocument();
    expect(screen.getByRole('button', { name: /download fit/i })).toBeInTheDocument();
  });

  it('renders completed metrics from event actual workout when detailed activity is unavailable', async () => {
    vi.mocked(loadEvent).mockResolvedValue({
      id: 22,
      startDateLocal: '2026-03-26',
      name: 'Over-Unders',
      category: 'WORKOUT',
      description: null,
      indoor: false,
      color: null,
      eventDefinition: {
        rawWorkoutDoc: '- 4x6min',
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
        activityId: 'a22',
        activityName: 'Executed Over-Unders',
        startDateLocal: '2026-03-26T08:00:00',
        powerValues: [200, 260, 310],
        cadenceValues: [85, 89, 92],
        heartRateValues: [140, 155, 166],
        speedValues: [8.5, 9.4, 10.2],
        averagePowerWatts: 255,
        normalizedPowerWatts: 272,
        trainingStressScore: 81,
        intensityFactor: 0.91,
        complianceScore: 0.87,
        matchedIntervals: [],
      },
    });
    vi.mocked(loadActivity).mockRejectedValue(new Error('activity fetch failed'));

    const selection: WorkoutDetailSelection = {
      dateKey: '2026-03-26',
      event: {
        id: 22,
        startDateLocal: '2026-03-26',
        name: 'Over-Unders',
        category: 'WORKOUT',
        description: null,
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
      },
      activity: {
        id: 'missing-activity',
        startDateLocal: '2026-03-26T08:00:00',
        startDate: '2026-03-26T07:00:00Z',
        name: null,
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
      },
    };

    render(<WorkoutDetailModal apiBaseUrl="" selection={selection} onClose={vi.fn()} />);

    await waitFor(() => {
      expect(screen.getByText(/completed workout/i)).toBeInTheDocument();
    });

    expect(screen.getByText('Executed Over-Unders')).toBeInTheDocument();
    expect(screen.getByText('272 W')).toBeInTheDocument();
    expect(screen.getByText('81 TSS')).toBeInTheDocument();
    expect(screen.getByText(/87% compliance/i)).toBeInTheDocument();
  });

  it('hides FIT download action in completed mode', async () => {
    vi.mocked(loadEvent).mockResolvedValue({
      id: 23,
      startDateLocal: '2026-03-26',
      name: 'Completed Workout',
      category: 'WORKOUT',
      description: null,
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
        activityId: 'a23',
        activityName: 'Done Ride',
        startDateLocal: '2026-03-26T08:00:00',
        powerValues: [220],
        cadenceValues: [88],
        heartRateValues: [150],
        speedValues: [9.1],
        averagePowerWatts: 220,
        normalizedPowerWatts: 225,
        trainingStressScore: 50,
        intensityFactor: 0.8,
        complianceScore: 0.8,
        matchedIntervals: [],
      },
    });
    vi.mocked(loadActivity).mockResolvedValue(undefined as never);

    render(<WorkoutDetailModal apiBaseUrl="" selection={{ dateKey: '2026-03-26', event: { id: 23, startDateLocal: '2026-03-26', name: 'Completed Workout', category: 'WORKOUT', description: null, indoor: false, color: null, eventDefinition: { rawWorkoutDoc: null, intervals: [], segments: [], summary: { totalSegments: 0, totalDurationSeconds: 0, estimatedNormalizedPowerWatts: null, estimatedAveragePowerWatts: null, estimatedIntensityFactor: null, estimatedTrainingStressScore: null } }, actualWorkout: null }, activity: null }} onClose={vi.fn()} />);

    await waitFor(() => {
      expect(screen.getByText(/completed workout/i)).toBeInTheDocument();
    });

    expect(screen.queryByRole('button', { name: /download fit/i })).not.toBeInTheDocument();
  });

  it('downloads the event FIT file from the modal action', async () => {
    vi.mocked(loadEvent).mockResolvedValue({
      id: 31,
      startDateLocal: '2026-03-26',
      name: 'Race Prep',
      category: 'WORKOUT',
      description: null,
      indoor: true,
      color: null,
      eventDefinition: {
        rawWorkoutDoc: '- 60min endurance',
        intervals: [],
        segments: [],
        summary: {
          totalSegments: 0,
          totalDurationSeconds: 3600,
          estimatedNormalizedPowerWatts: null,
          estimatedAveragePowerWatts: null,
          estimatedIntensityFactor: null,
          estimatedTrainingStressScore: null,
        },
      },
      actualWorkout: null,
    });
    vi.mocked(loadActivity).mockResolvedValue(undefined as never);
    vi.mocked(downloadFit).mockResolvedValue(new Uint8Array([1, 2, 3]));

    const createObjectURL = vi.fn(() => 'blob:fit-download');
    const revokeObjectURL = vi.fn();
    const originalCreateObjectURL = URL.createObjectURL;
    const originalRevokeObjectURL = URL.revokeObjectURL;
    URL.createObjectURL = createObjectURL;
    URL.revokeObjectURL = revokeObjectURL;

    const click = vi.fn();
    const originalCreateElement = document.createElement.bind(document);
    const createElementSpy = vi.spyOn(document, 'createElement').mockImplementation(((tagName: string) => {
      const element = originalCreateElement(tagName);
      if (tagName === 'a') {
        Object.defineProperty(element, 'click', {
          configurable: true,
          value: click,
        });
      }
      return element;
    }) as typeof document.createElement);

    const selection: WorkoutDetailSelection = {
      dateKey: '2026-03-26',
      event: {
        id: 31,
        startDateLocal: '2026-03-26',
        name: 'Race Prep',
        category: 'WORKOUT',
        description: null,
        indoor: true,
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
      },
      activity: null,
    };

    render(<WorkoutDetailModal apiBaseUrl="" selection={selection} onClose={vi.fn()} />);

    await waitFor(() => {
      expect(screen.getByText('Race Prep')).toBeInTheDocument();
    });

    await userEvent.click(screen.getByRole('button', { name: /download fit/i }));

    await waitFor(() => {
      expect(downloadFit).toHaveBeenCalledWith('', 31);
    });

    expect(createObjectURL).toHaveBeenCalledTimes(1);
    expect(click).toHaveBeenCalledTimes(1);
    expect(revokeObjectURL).toHaveBeenCalledWith('blob:fit-download');

    createElementSpy.mockRestore();
    URL.createObjectURL = originalCreateObjectURL;
    URL.revokeObjectURL = originalRevokeObjectURL;
  });
});
