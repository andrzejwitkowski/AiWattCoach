import { cleanup, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import {
  makeActivity,
  makeActivityInterval,
  makeActivityStream,
  makeActualWorkout,
  makeEvent,
  makeEventDefinition,
  makeIntervalDefinition,
  makeSelection,
  makeWorkoutSegment,
  makeWorkoutSummary,
} from '../testData';
import { metricCard, mockedLoadActivity, mockedLoadEvent } from './WorkoutDetailModal.testHelpers';
import { WorkoutDetailModal } from './WorkoutDetailModal';

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

describe('WorkoutDetailModal completed mode', () => {
  it('loads and renders a completed workout detail view with comparison data', async () => {
    mockedLoadEvent.mockResolvedValue(
      makeEvent({
        id: 21,
        name: 'Threshold',
        indoor: false,
        eventDefinition: makeEventDefinition({
          rawWorkoutDoc: '- 20min 95%',
          intervals: [makeIntervalDefinition({ definition: '- 20min 95%', durationSeconds: 1200, targetPercentFtp: 95, zoneId: 4 })],
          segments: [makeWorkoutSegment({ label: '20min 95%', durationSeconds: 1200, endOffsetSeconds: 1200, targetPercentFtp: 95, zoneId: 4 })],
          summary: makeWorkoutSummary({
            totalSegments: 1,
            totalDurationSeconds: 1200,
            estimatedNormalizedPowerWatts: 285,
            estimatedAveragePowerWatts: 285,
            estimatedIntensityFactor: 0.95,
            estimatedTrainingStressScore: 30.1,
          }),
        }),
        actualWorkout: makeActualWorkout({
          activityId: 'a21',
          activityName: 'Threshold Ride',
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
            },
          ],
        }),
      }),
    );
    mockedLoadActivity.mockResolvedValue(
      makeActivity({
        id: 'a21',
        name: 'Threshold Ride',
        distanceMeters: 40000,
        movingTimeSeconds: 3600,
        elapsedTimeSeconds: 3650,
        averageSpeedMps: 10.1,
        averageHeartRateBpm: 158,
        averageCadenceRpm: 89,
        hasHeartRate: true,
        streamTypes: ['watts', 'heartrate', 'cadence'],
        metrics: { trainingStressScore: 78, normalizedPowerWatts: 280, intensityFactor: 0.93, averagePowerWatts: 271, ftpWatts: 300 },
        details: {
          streams: [
            makeActivityStream({ streamType: 'watts', data: [150, 210, 290, 275] }),
            makeActivityStream({ streamType: 'heartrate', name: 'Heart Rate', data: [130, 145, 162, 165] }),
            makeActivityStream({ streamType: 'cadence', name: 'Cadence', data: [85, 88, 92, 89] }),
          ],
        },
      }),
    );

    const onClose = vi.fn();
    render(
      <WorkoutDetailModal
        apiBaseUrl=""
        selection={makeSelection({
          event: makeEvent({ id: 21, name: 'Threshold', indoor: false }),
          activity: makeActivity({ id: 'a21', name: 'Threshold Ride', hasHeartRate: true }),
        })}
        onClose={onClose}
      />,
    );

    await waitFor(() => expect(screen.getByText(/completed workout/i)).toBeInTheDocument());

    expect(screen.getByText('Threshold Ride')).toBeInTheDocument();
    expect(within(metricCard('Duration')).getByText('1h 00m')).toBeInTheDocument();
    expect(within(metricCard('NP')).getByText('280 W')).toBeInTheDocument();
    expect(within(metricCard('TSS')).getByText('35 TSS')).toBeInTheDocument();
    expect(screen.queryByText('78 TSS')).not.toBeInTheDocument();
    expect(screen.getAllByText(/91% compliance/i)).toHaveLength(2);
    expect(document.querySelectorAll('[data-chart-bar="detail"]')).toHaveLength(1);
    const [bar] = Array.from(document.querySelectorAll('[data-chart-bar="detail"]')) as HTMLDivElement[];
    expect(bar.style.flexGrow).toBe('1200');

    await screen.getByRole('button', { name: /close workout details/i }).click();
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it('keeps selected activity details visible when activity reload fails for an activity-only day', async () => {
    mockedLoadEvent.mockResolvedValue(undefined as never);
    mockedLoadActivity.mockRejectedValue(new Error('activity fetch failed'));

    render(
      <WorkoutDetailModal
        apiBaseUrl=""
        selection={makeSelection({
          dateKey: '2026-03-27',
          activity: makeActivity({
            id: 'a24',
            startDateLocal: '2026-03-27T08:00:00',
            name: 'Solo ride',
            movingTimeSeconds: 2700,
            elapsedTimeSeconds: 2750,
            hasHeartRate: false,
            streamTypes: ['watts'],
            metrics: { trainingStressScore: 62, normalizedPowerWatts: 228, intensityFactor: 0.81, averagePowerWatts: 219, ftpWatts: 280 },
            details: { streams: [makeActivityStream({ data: [180, 220, 260] })] },
          }),
        })}
        onClose={vi.fn()}
      />,
    );

    await waitFor(() => expect(screen.getByText(/completed workout/i)).toBeInTheDocument());

    expect(screen.getByText('Solo ride')).toBeInTheDocument();
    expect(screen.getByText('228 W')).toBeInTheDocument();
    expect(screen.getByText('62 TSS')).toBeInTheDocument();
  });

  it('renders completed metrics from event actual workout when detailed activity is unavailable', async () => {
    mockedLoadEvent.mockResolvedValue(
      makeEvent({
        id: 22,
        startDateLocal: '2026-03-26',
        name: 'Over-Unders',
        indoor: false,
        eventDefinition: makeEventDefinition({ rawWorkoutDoc: '- 4x6min' }),
        actualWorkout: makeActualWorkout({
          activityId: 'a22',
          activityName: 'Executed Over-Unders',
          powerValues: [200, 260, 310],
          cadenceValues: [85, 89, 92],
          heartRateValues: [140, 155, 166],
          speedValues: [8.5, 9.4, 10.2],
          averagePowerWatts: 255,
          normalizedPowerWatts: 272,
          trainingStressScore: 81,
          intensityFactor: 0.91,
          complianceScore: 0.87,
        }),
      }),
    );
    mockedLoadActivity.mockRejectedValue(new Error('activity fetch failed'));

    render(
      <WorkoutDetailModal
        apiBaseUrl=""
        selection={makeSelection({
          dateKey: '2026-03-26',
          event: makeEvent({ id: 22, startDateLocal: '2026-03-26', name: 'Over-Unders', indoor: false }),
          activity: makeActivity({ id: 'missing-activity', startDateLocal: '2026-03-26T08:00:00', name: null, hasHeartRate: false }),
        })}
        onClose={vi.fn()}
      />,
    );

    await waitFor(() => expect(screen.getByText(/completed workout/i)).toBeInTheDocument());

    expect(screen.getByText('Executed Over-Unders')).toBeInTheDocument();
    expect(screen.getByText('272 W')).toBeInTheDocument();
    expect(screen.getByText('81 TSS')).toBeInTheDocument();
    expect(screen.getByText(/87% compliance/i)).toBeInTheDocument();
    expect(within(metricCard('Duration')).getByText('0m')).toBeInTheDocument();
  });

  it('shows imported activity details unavailable hint for sparse completed imports', async () => {
    mockedLoadEvent.mockResolvedValue(undefined as never);
    mockedLoadActivity.mockResolvedValue(
      makeActivity({
        id: 'a24',
        startDateLocal: '2026-03-26T08:00:00',
        name: 'Imported Ride',
        source: 'STRAVA',
        distanceMeters: 40200,
        movingTimeSeconds: 3600,
        elapsedTimeSeconds: 3700,
        detailsUnavailableReason: 'Intervals.icu did not provide detailed data for this imported activity.',
      }),
    );

    render(
      <WorkoutDetailModal
        apiBaseUrl=""
        selection={makeSelection({
          dateKey: '2026-03-26',
          activity: makeActivity({
            id: 'a24',
            startDateLocal: '2026-03-26T08:00:00',
            name: 'Imported Ride',
            source: 'STRAVA',
            distanceMeters: 40200,
            movingTimeSeconds: 3600,
            elapsedTimeSeconds: 3700,
          }),
        })}
        onClose={vi.fn()}
      />,
    );

    await waitFor(() => expect(screen.getByText('Imported Ride')).toBeInTheDocument());

    expect(screen.getByText('Intervals.icu did not provide detailed data for this imported activity.')).toBeInTheDocument();
  });

  it('renders completed activity bars from skyline chart payloads', async () => {
    mockedLoadEvent.mockResolvedValue(undefined as never);
    mockedLoadActivity.mockResolvedValue(
      makeActivity({
        id: 'a26',
        startDateLocal: '2026-03-26T08:00:00',
        name: 'Skyline Import',
        source: 'STRAVA',
        movingTimeSeconds: 3600,
        elapsedTimeSeconds: 3600,
        details: {
          skylineChart: ['CAcSAtJFGgFAIgECKAE='],
        },
      }),
    );

    render(
      <WorkoutDetailModal
        apiBaseUrl=""
        selection={makeSelection({
          dateKey: '2026-03-26',
          activity: makeActivity({
            id: 'a26',
            startDateLocal: '2026-03-26T08:00:00',
            name: 'Skyline Import',
            source: 'STRAVA',
          }),
        })}
        onClose={vi.fn()}
      />,
    );

    await waitFor(() => expect(screen.getByText('Skyline Import')).toBeInTheDocument());

    const [bar] = Array.from(document.querySelectorAll('[data-chart-bar="detail"]')) as HTMLDivElement[];
    expect(bar).toBeDefined();
    expect(bar.style.flexGrow).toBe('82');
    expect(bar.style.height).toBe('64%');
  });

  it('shows seconds for sub-minute completed intervals', async () => {
    mockedLoadEvent.mockResolvedValue(undefined as never);
    mockedLoadActivity.mockResolvedValue(
      makeActivity({
        id: 'a55',
        startDateLocal: '2026-04-05T08:00:00',
        name: 'Sprint Ride',
        distanceMeters: 10000,
        movingTimeSeconds: 1800,
        elapsedTimeSeconds: 1800,
        hasHeartRate: true,
        streamTypes: ['watts'],
        metrics: { trainingStressScore: 20, normalizedPowerWatts: 230, intensityFactor: 0.8, averagePowerWatts: 220, ftpWatts: 280 },
        details: {
          intervals: [makeActivityInterval({ label: 'Sprint', endIndex: 44, endTimeSeconds: 45, movingTimeSeconds: 45, elapsedTimeSeconds: 45, averagePowerWatts: 500, averageHeartRateBpm: 160, zone: 6 })],
          streams: [makeActivityStream({ data: [300, 400, 500] })],
        },
      }),
    );

    render(
      <WorkoutDetailModal
        apiBaseUrl=""
        selection={makeSelection({
          dateKey: '2026-04-05',
          activity: makeActivity({ id: 'a55', startDateLocal: '2026-04-05T08:00:00', name: 'Sprint Ride', movingTimeSeconds: 1800, elapsedTimeSeconds: 1800, hasHeartRate: true }),
        })}
        onClose={vi.fn()}
      />,
    );

    await waitFor(() => expect(screen.getByText(/completed workout/i)).toBeInTheDocument());

    expect(screen.getByText('45s')).toBeInTheDocument();
  });
});
