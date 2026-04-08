import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import {
  makeActivity,
  makeActivityInterval,
  makeActivityStream,
  makeActualWorkout,
  makeEvent,
  makeEventDefinition,
  makeIntervalDefinition,
  makeMatchedInterval,
  makeSelection,
  makeWorkoutSegment,
  makeWorkoutSummary,
} from '../testData';
import { mockedLoadActivity, mockedLoadEvent, setChartRect } from './WorkoutDetailModal.testHelpers';
import { WorkoutDetailModal } from './WorkoutDetailModal';

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

describe('WorkoutDetailModal charts and interaction', () => {
  it('renders enriched completed-only title duration metrics and chart bars from activity payloads', async () => {
    mockedLoadEvent.mockResolvedValue(undefined as never);
    mockedLoadActivity.mockResolvedValue(
      makeActivity({
        id: 'a27',
        startDateLocal: '2026-03-30T06:45:00',
        name: null,
        source: 'STRAVA',
        distanceMeters: 42000,
        movingTimeSeconds: 0,
        elapsedTimeSeconds: 5520,
        averageSpeedMps: 7.8,
        averageHeartRateBpm: 144,
        averageCadenceRpm: 86,
        hasHeartRate: true,
        streamTypes: ['watts'],
        metrics: { trainingStressScore: 67, normalizedPowerWatts: 238, intensityFactor: 0.79, averagePowerWatts: 224, ftpWatts: 300 },
        details: {
          intervals: [
            makeActivityInterval({ id: 1, label: 'Steady Block', groupId: 'steady', startIndex: 0, endIndex: 1199, startTimeSeconds: 0, endTimeSeconds: 1200, movingTimeSeconds: 1200, elapsedTimeSeconds: 1200, distanceMeters: 9000, averagePowerWatts: 228, normalizedPowerWatts: 236, trainingStressScore: 15.2, averageHeartRateBpm: 142, averageCadenceRpm: 85, averageSpeedMps: 7.6, zone: 3 }),
            makeActivityInterval({ id: 2, label: 'Finish Block', groupId: 'steady', startIndex: 1200, endIndex: 2399, startTimeSeconds: 1200, endTimeSeconds: 2400, movingTimeSeconds: 1200, elapsedTimeSeconds: 1200, distanceMeters: 9200, averagePowerWatts: 232, normalizedPowerWatts: 240, trainingStressScore: 15.6, averageHeartRateBpm: 145, averageCadenceRpm: 86, averageSpeedMps: 7.8, zone: 3 }),
          ],
          streams: [makeActivityStream({ data: [180, 220, 245, 235, 250] })],
        },
      }),
    );

    const { container } = render(
      <WorkoutDetailModal
        apiBaseUrl=""
        selection={makeSelection({
          dateKey: '2026-03-30',
          activity: makeActivity({ id: 'a27', startDateLocal: '2026-03-30T06:45:00', name: null, movingTimeSeconds: 0, elapsedTimeSeconds: 0, hasHeartRate: true }),
        })}
        onClose={vi.fn()}
      />,
    );

    await waitFor(() => expect(screen.getByText(/completed workout/i)).toBeInTheDocument());

    expect(screen.getByRole('heading', { name: 'Ride' })).toBeInTheDocument();
    expect(screen.queryByRole('heading', { name: 'Workout' })).not.toBeInTheDocument();
    expect(screen.getByText('1h 32m')).toBeInTheDocument();
    expect(screen.getByText('238 W')).toBeInTheDocument();
    expect(screen.getByText('67 TSS')).toBeInTheDocument();
    const detailBars = Array.from(container.querySelectorAll('[data-chart-bar="detail"]')) as HTMLDivElement[];
    expect(detailBars).toHaveLength(2);
    expect(detailBars[0].style.flexGrow).toBe('1200');
    expect(detailBars[1].style.flexGrow).toBe('1200');
    expect(screen.getByLabelText(/power chart/i)).toBeInTheDocument();
    expect(screen.getByText('226 W max (5s avg)')).toBeInTheDocument();
    expect(container.querySelectorAll('[data-interval-overlay="true"]')).toHaveLength(2);
  });

  it('renders a power chart for completed comparison workouts from actual power values', async () => {
    mockedLoadEvent.mockResolvedValue(
      makeEvent({
        id: 44,
        startDateLocal: '2026-03-30',
        name: 'Tempo Build',
        indoor: false,
        eventDefinition: makeEventDefinition({
          rawWorkoutDoc: '- 2x10min 90%',
          intervals: [makeIntervalDefinition({ definition: '- 2x10min 90%', repeatCount: 2, durationSeconds: 600, targetPercentFtp: 90, zoneId: 3 })],
          segments: [makeWorkoutSegment({ label: 'Tempo', durationSeconds: 1200, endOffsetSeconds: 1200, targetPercentFtp: 90, zoneId: 3 })],
          summary: makeWorkoutSummary({ totalSegments: 1, totalDurationSeconds: 1200, estimatedNormalizedPowerWatts: 265, estimatedAveragePowerWatts: 255, estimatedIntensityFactor: 0.9, estimatedTrainingStressScore: 32 }),
        }),
        actualWorkout: makeActualWorkout({
          activityId: 'a44',
          activityName: 'Tempo Build Outside',
          startDateLocal: '2026-03-30T07:00:00',
          powerValues: [160, 210, 245, 265, 238],
          cadenceValues: [80, 85, 90, 88, 86],
          heartRateValues: [128, 138, 149, 153, 151],
          speedValues: [8.1, 9.0, 9.8, 9.7, 9.5],
          averagePowerWatts: 224,
          normalizedPowerWatts: 239,
          trainingStressScore: 36,
          intensityFactor: 0.82,
          complianceScore: 0.88,
        }),
      }),
    );
    mockedLoadActivity.mockResolvedValue(undefined as never);

    render(
      <WorkoutDetailModal
        apiBaseUrl=""
        selection={makeSelection({ dateKey: '2026-03-30', event: makeEvent({ id: 44, startDateLocal: '2026-03-30', name: 'Tempo Build', indoor: false }) })}
        onClose={vi.fn()}
      />,
    );

    await waitFor(() => expect(screen.getByText(/completed workout/i)).toBeInTheDocument());

    expect(screen.getByLabelText(/power chart/i)).toBeInTheDocument();
    expect(screen.getByText('224 W max (5s avg)')).toBeInTheDocument();
    expect(screen.queryAllByText('Tempo').length).toBeGreaterThan(0);
    expect(screen.getByText('0:00')).toBeInTheDocument();
    expect(screen.getByText('19:59')).toBeInTheDocument();
  });

  it('renders power chart from 5 second average activity values', async () => {
    mockedLoadEvent.mockResolvedValue(undefined as never);
    mockedLoadActivity.mockResolvedValue(
      makeActivity({
        id: 'a-5s',
        details: {
          streams: [makeActivityStream({ data: [100, 150, 200, 250, 300, 350, 400, 450, 500, 550] })],
        },
      }),
    );

    render(
      <WorkoutDetailModal
        apiBaseUrl=""
        selection={makeSelection({ dateKey: '2026-03-30', activity: makeActivity({ id: 'a-5s' }) })}
        onClose={vi.fn()}
      />,
    );

    await waitFor(() => expect(screen.getByLabelText(/power chart/i)).toBeInTheDocument());

    expect(screen.getByText('450 W max (5s avg)')).toBeInTheDocument();
  });

  it('preserves the original max value label when long series are downsampled', async () => {
    mockedLoadEvent.mockResolvedValue(undefined as never);
    mockedLoadActivity.mockResolvedValue(
      makeActivity({
        id: 'a-max',
        details: {
          streams: [
            makeActivityStream({
              data: Array.from({ length: 500 }, (_, index) => (index >= 320 && index <= 324 ? 999 : 120)),
            }),
          ],
        },
      }),
    );

    render(
      <WorkoutDetailModal
        apiBaseUrl=""
        selection={makeSelection({ dateKey: '2026-03-30', activity: makeActivity({ id: 'a-max' }) })}
        onClose={vi.fn()}
      />,
    );

    await waitFor(() => expect(screen.getByLabelText(/power chart/i)).toBeInTheDocument());

    expect(screen.getByText('999 W max (5s avg)')).toBeInTheDocument();
  });

  it('shows hovered power readout next to the max power label', async () => {
    mockedLoadEvent.mockResolvedValue(
      makeEvent({
        id: 46,
        startDateLocal: '2026-04-01',
        name: 'Hover Test',
        indoor: false,
        eventDefinition: makeEventDefinition({
          rawWorkoutDoc: '- 5min tempo',
          segments: [makeWorkoutSegment({ label: 'Tempo', durationSeconds: 5, endOffsetSeconds: 5, targetPercentFtp: 90, zoneId: 3 })],
          summary: makeWorkoutSummary({ totalSegments: 1, totalDurationSeconds: 5, estimatedNormalizedPowerWatts: 230, estimatedAveragePowerWatts: 225, estimatedIntensityFactor: 0.8, estimatedTrainingStressScore: 4 }),
        }),
        actualWorkout: makeActualWorkout({ activityId: 'a46', activityName: 'Hover Ride', startDateLocal: '2026-04-01T07:00:00', powerValues: [100, 150, 200, 250, 300], averagePowerWatts: 200, normalizedPowerWatts: 220, trainingStressScore: 20, intensityFactor: 0.73, complianceScore: 0.8 }),
      }),
    );
    mockedLoadActivity.mockResolvedValue(undefined as never);

    render(<WorkoutDetailModal apiBaseUrl="" selection={makeSelection({ dateKey: '2026-04-01', event: makeEvent({ id: 46, startDateLocal: '2026-04-01', name: 'Hover Test', indoor: false }) })} onClose={vi.fn()} />);

    await waitFor(() => expect(screen.getByLabelText(/power chart/i)).toBeInTheDocument());

    const chart = screen.getByLabelText(/power chart/i);
    setChartRect(chart);
    fireEvent.mouseMove(chart, { clientX: 500, clientY: 80 });

    expect(screen.getByText('200 W max (5s avg)')).toBeInTheDocument();
    expect(screen.getByText((content) => content.includes('0:02') && content.includes('150') && content.includes('W'))).toBeInTheDocument();
  });

  it('renders comparison workout bars with width proportional to matched interval durations', async () => {
    mockedLoadEvent.mockResolvedValue(
      makeEvent({
        id: 45,
        startDateLocal: '2026-03-31',
        name: 'Mixed durations',
        indoor: false,
        eventDefinition: makeEventDefinition({ summary: makeWorkoutSummary({ totalDurationSeconds: 1500 }) }),
        actualWorkout: makeActualWorkout({
          activityId: 'a45',
          activityName: 'Mixed durations ride',
          startDateLocal: '2026-03-31T07:00:00',
          powerValues: [180, 220, 260, 280, 230],
          averagePowerWatts: 230,
          normalizedPowerWatts: 242,
          trainingStressScore: 40,
          intensityFactor: 0.84,
          complianceScore: 0.9,
          matchedIntervals: [
            makeMatchedInterval({ plannedLabel: 'Long', plannedDurationSeconds: 1200, targetPercentFtp: 90, zoneId: 4, actualEndTimeSeconds: 1200, averagePowerWatts: 250, normalizedPowerWatts: 255, complianceScore: 0.92 }),
            makeMatchedInterval({ plannedSegmentOrder: 1, plannedLabel: 'Short', plannedDurationSeconds: 300, targetPercentFtp: 80, zoneId: 3, actualStartTimeSeconds: 1200, actualEndTimeSeconds: 1500, averagePowerWatts: 210, normalizedPowerWatts: 215, complianceScore: 0.86 }),
          ],
        }),
      }),
    );
    mockedLoadActivity.mockResolvedValue(undefined as never);

    render(<WorkoutDetailModal apiBaseUrl="" selection={makeSelection({ dateKey: '2026-03-31', event: makeEvent({ id: 45, startDateLocal: '2026-03-31', name: 'Mixed durations', indoor: false }) })} onClose={vi.fn()} />);

    await waitFor(() => expect(screen.getByText(/completed workout/i)).toBeInTheDocument());

    const detailBars = Array.from(document.querySelectorAll('[data-chart-bar="detail"]')) as HTMLDivElement[];
    expect(detailBars).toHaveLength(2);
    expect(detailBars[0].style.flexGrow).toBe('1200');
    expect(detailBars[1].style.flexGrow).toBe('300');
    expect(document.querySelectorAll('[data-interval-overlay="true"]')).toHaveLength(2);
  });
});
