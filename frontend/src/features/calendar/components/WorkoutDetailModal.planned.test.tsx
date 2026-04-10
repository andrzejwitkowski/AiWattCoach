import { cleanup, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import {
  makeActivity,
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

describe('WorkoutDetailModal planned mode', () => {
  it('loads and renders a planned workout detail view', async () => {
    mockedLoadEvent.mockResolvedValue(
      makeEvent({
        id: 11,
        name: 'Sweet Spot',
        eventDefinition: makeEventDefinition({
          rawWorkoutDoc: '- 3x8min 95%',
          intervals: [makeIntervalDefinition({ definition: '- 3x8min 95%', repeatCount: 3, durationSeconds: 480, targetPercentFtp: 95, zoneId: 4 })],
          segments: Array.from({ length: 3 }, (_, index) =>
            makeWorkoutSegment({
              order: index,
              label: `3x8min 95% #${index + 1}`,
              durationSeconds: 480,
              startOffsetSeconds: index * 480,
              endOffsetSeconds: (index + 1) * 480,
              targetPercentFtp: 95,
              zoneId: 4,
            }),
          ),
          summary: makeWorkoutSummary({
            totalSegments: 3,
            totalDurationSeconds: 1440,
            estimatedNormalizedPowerWatts: 285,
            estimatedAveragePowerWatts: 278,
            estimatedIntensityFactor: 0.95,
            estimatedTrainingStressScore: 38,
          }),
        }),
      }),
    );
    mockedLoadActivity.mockResolvedValue(undefined as never);

    render(
      <WorkoutDetailModal
        apiBaseUrl=""
        selection={makeSelection({ event: makeEvent({ id: 11, name: 'Sweet Spot' }) })}
        onClose={vi.fn()}
      />,
    );

    await waitFor(() => expect(within(metricCard('Duration')).getByText('24m')).toBeInTheDocument());

    expect(screen.getByText(/planned workout/i)).toBeInTheDocument();
    expect(within(metricCard('Duration')).getByText('24m')).toBeInTheDocument();
    expect(screen.getByText(/0.95 IF/i)).toBeInTheDocument();
    expect(screen.getByText(/workout structure/i)).toBeInTheDocument();
    expect(screen.getByText('3 x 8min 95% FTP')).toBeInTheDocument();
  });

  it('keeps planned workout details visible when activity loading fails', async () => {
    mockedLoadEvent.mockResolvedValue(
      makeEvent({
        id: 12,
        name: 'Tempo Builder',
        eventDefinition: makeEventDefinition({
          rawWorkoutDoc: '- 40min 80%',
          intervals: [makeIntervalDefinition({ definition: '- 40min 80%', durationSeconds: 2400, targetPercentFtp: 80, zoneId: 3 })],
          segments: [makeWorkoutSegment({ label: '40min 80%', durationSeconds: 2400, endOffsetSeconds: 2400, targetPercentFtp: 80, zoneId: 3 })],
          summary: makeWorkoutSummary({
            totalSegments: 1,
            totalDurationSeconds: 2400,
            estimatedNormalizedPowerWatts: 240,
            estimatedAveragePowerWatts: 235,
            estimatedIntensityFactor: 0.8,
            estimatedTrainingStressScore: 42,
          }),
        }),
      }),
    );
    mockedLoadActivity.mockRejectedValue(new Error('activity fetch failed'));

    render(
      <WorkoutDetailModal
        apiBaseUrl=""
        selection={makeSelection({
          event: makeEvent({ id: 12, name: 'Tempo Builder' }),
          activity: makeActivity({ id: 'missing-activity', name: 'Missing Activity', hasHeartRate: false }),
        })}
        onClose={vi.fn()}
      />,
    );

    await waitFor(() => expect(within(metricCard('Duration')).getByText('40m')).toBeInTheDocument());

    expect(screen.getByText(/planned workout/i)).toBeInTheDocument();
    expect(within(metricCard('Duration')).getByText('40m')).toBeInTheDocument();
    expect(screen.getByText(/0.80 IF/i)).toBeInTheDocument();
    expect(within(metricCard('TSS')).getByText('42 TSS')).toBeInTheDocument();
    expect(within(metricCard('NP')).getByText('240 W')).toBeInTheDocument();
    expect(screen.queryByText('activity fetch failed')).not.toBeInTheDocument();
    expect(document.querySelectorAll('[data-chart-bar="detail"]')).toHaveLength(1);
  });

  it('stays in planned mode when an unrelated selected activity exists', async () => {
    mockedLoadEvent.mockResolvedValue(
      makeEvent({
        id: 32,
        name: 'Plan only',
        eventDefinition: makeEventDefinition({
          rawWorkoutDoc: '- 30min 85%',
          intervals: [makeIntervalDefinition({ definition: '- 30min 85%', durationSeconds: 1800, targetPercentFtp: 85, zoneId: 3 })],
          segments: [makeWorkoutSegment({ label: '30min 85%', durationSeconds: 1800, endOffsetSeconds: 1800, targetPercentFtp: 85, zoneId: 3 })],
          summary: makeWorkoutSummary({
            totalSegments: 1,
            totalDurationSeconds: 1800,
            estimatedNormalizedPowerWatts: 255,
            estimatedAveragePowerWatts: 255,
            estimatedIntensityFactor: 0.85,
            estimatedTrainingStressScore: 36.1,
          }),
        }),
      }),
    );
    mockedLoadActivity.mockResolvedValue(
      makeActivity({
        id: 'a-unrelated',
        name: 'Unrelated ride',
        movingTimeSeconds: 2700,
        elapsedTimeSeconds: 2750,
        metrics: { trainingStressScore: 62, normalizedPowerWatts: 228, intensityFactor: 0.81, averagePowerWatts: 219, ftpWatts: 280 },
      }),
    );

    render(
      <WorkoutDetailModal
        apiBaseUrl=""
        selection={makeSelection({
          dateKey: '2026-03-27',
          event: makeEvent({ id: 32, startDateLocal: '2026-03-27', name: 'Plan only' }),
          activity: makeActivity({
            id: 'a-unrelated',
            startDateLocal: '2026-03-27T08:00:00',
            name: 'Unrelated ride',
            movingTimeSeconds: 1200,
            elapsedTimeSeconds: 1260,
            metrics: { trainingStressScore: 18, normalizedPowerWatts: 180, intensityFactor: 0.64, averagePowerWatts: 172, ftpWatts: 280 },
          }),
        })}
        onClose={vi.fn()}
      />,
    );

    await waitFor(() => expect(within(metricCard('Duration')).getByText('30m')).toBeInTheDocument());

    expect(screen.getByText(/planned workout/i)).toBeInTheDocument();
    expect(screen.queryByText('Unrelated ride')).not.toBeInTheDocument();
    expect(within(metricCard('Duration')).getByText('30m')).toBeInTheDocument();
    expect(within(metricCard('IF')).getByText('0.85 IF')).toBeInTheDocument();
    expect(within(metricCard('TSS')).getByText('36 TSS')).toBeInTheDocument();
    expect(within(metricCard('NP')).getByText('255 W')).toBeInTheDocument();
    expect(screen.queryByText('18 TSS')).not.toBeInTheDocument();
    expect(screen.queryByText('228 W')).not.toBeInTheDocument();
    expect(screen.getByRole('button', { name: /download fit/i })).toBeInTheDocument();
  });

  it('renders the planned chart and grouped workout structure for repeat blocks', async () => {
    mockedLoadEvent.mockResolvedValue(
      makeEvent({
        id: 77,
        name: 'VO2 Builder',
        eventDefinition: makeEventDefinition({
          rawWorkoutDoc: 'Main Set\n- 4x120% ftp 2min and 2min of rest 50%',
          intervals: [
            makeIntervalDefinition({
              definition: '- 4x120% ftp 2min and 2min of rest 50%',
              repeatCount: 4,
              durationSeconds: 240,
              targetPercentFtp: 120,
              zoneId: 5,
            }),
          ],
          segments: [
            makeWorkoutSegment({
              order: 0,
              label: 'Work',
              durationSeconds: 120,
              startOffsetSeconds: 0,
              endOffsetSeconds: 120,
              targetPercentFtp: 120,
              zoneId: 5,
            }),
            makeWorkoutSegment({
              order: 1,
              label: 'Rest',
              durationSeconds: 120,
              startOffsetSeconds: 120,
              endOffsetSeconds: 240,
              targetPercentFtp: 50,
              zoneId: 1,
            }),
          ],
          summary: makeWorkoutSummary({
            totalSegments: 2,
            totalDurationSeconds: 960,
            estimatedNormalizedPowerWatts: 310,
            estimatedAveragePowerWatts: 280,
            estimatedIntensityFactor: 0.94,
            estimatedTrainingStressScore: 44,
          }),
        }),
      }),
    );
    mockedLoadActivity.mockResolvedValue(undefined as never);

    render(
      <WorkoutDetailModal
        apiBaseUrl=""
        selection={makeSelection({
          event: makeEvent({ id: 77, name: 'VO2 Builder' }),
        })}
        onClose={vi.fn()}
      />,
    );

    await waitFor(() => expect(screen.getByLabelText(/power chart/i)).toBeInTheDocument());

    expect(screen.getByLabelText(/power chart/i)).toBeInTheDocument();
    expect(screen.getByText('120% FTP max target')).toBeInTheDocument();
    expect(screen.getByText('4 x 120% FTP 2min and 2min of rest 50% FTP')).toBeInTheDocument();
    expect(within(metricCard('Duration')).getByText('16m')).toBeInTheDocument();
  });

  it('preserves predicted sync metadata when linked event details are loaded', async () => {
    mockedLoadEvent.mockResolvedValue(
      makeEvent({
        id: 91,
        startDateLocal: '2026-03-26',
        name: 'Predicted Build',
        description: 'Intervals description',
        eventDefinition: makeEventDefinition({
          rawWorkoutDoc: '- 60min endurance',
          summary: makeWorkoutSummary({ totalDurationSeconds: 3600 }),
        }),
      }),
    );
    mockedLoadActivity.mockResolvedValue(undefined as never);

    render(
      <WorkoutDetailModal
        apiBaseUrl=""
        selection={makeSelection({
          dateKey: '2026-03-26',
          event: makeEvent({
            id: 901,
            startDateLocal: '2026-03-26',
            name: 'Predicted Build',
            plannedSource: 'predicted',
            syncStatus: 'synced',
            linkedIntervalsEventId: 91,
            projectedWorkout: {
              projectedWorkoutId: 'training-plan:user-1:w1:1:2026-03-26',
              operationKey: 'training-plan:user-1:w1:1',
              date: '2026-03-26',
              sourceWorkoutId: 'w1',
            },
            eventDefinition: makeEventDefinition({
              rawWorkoutDoc: '- 60min endurance',
              summary: makeWorkoutSummary({ totalDurationSeconds: 3600 }),
            }),
          }),
        })}
        onClose={vi.fn()}
      />,
    );

    await waitFor(() => expect(screen.getByRole('button', { name: /sync to intervals/i })).toBeInTheDocument());

    expect(screen.getByText(/planned workout/i)).toBeInTheDocument();
    expect(screen.getByText(/synced/i)).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /sync to intervals/i })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /download fit/i })).toBeInTheDocument();
    expect(screen.getByText('Predicted Build')).toBeInTheDocument();
  });

  it('preserves calendar event details when intervals event details are empty', async () => {
    mockedLoadEvent.mockResolvedValue(
      makeEvent({
        id: 41,
        startDateLocal: '2026-04-12',
        name: 'Opener Grojec',
        plannedSource: 'intervals',
        syncStatus: null,
        eventDefinition: makeEventDefinition(),
      }),
    );
    mockedLoadActivity.mockResolvedValue(undefined as never);

    render(
      <WorkoutDetailModal
        apiBaseUrl=""
        selection={makeSelection({
          dateKey: '2026-04-12',
          event: makeEvent({
            id: 41,
            startDateLocal: '2026-04-12',
            name: 'Opener Grojec',
            plannedSource: 'intervals',
            syncStatus: null,
            eventDefinition: makeEventDefinition({
              rawWorkoutDoc: '- 19min 55%',
              intervals: [makeIntervalDefinition({ definition: '- 19min 55%', durationSeconds: 1140, targetPercentFtp: 55, zoneId: 1 })],
              segments: [makeWorkoutSegment({ label: '19min 55%', durationSeconds: 1140, endOffsetSeconds: 1140, targetPercentFtp: 55, zoneId: 1 })],
              summary: makeWorkoutSummary({
                totalSegments: 1,
                totalDurationSeconds: 1140,
                estimatedIntensityFactor: 0.55,
                estimatedTrainingStressScore: 16,
              }),
            }),
          }),
        })}
        onClose={vi.fn()}
      />,
    );

    await waitFor(() => expect(within(metricCard('Duration')).getByText('19m')).toBeInTheDocument());

    expect(screen.getByText(/planned workout/i)).toBeInTheDocument();
    expect(within(metricCard('Duration')).getByText('19m')).toBeInTheDocument();
    expect(within(metricCard('IF')).getByText('0.55 IF')).toBeInTheDocument();
    expect(within(metricCard('TSS')).getByText('16 TSS')).toBeInTheDocument();
    expect(screen.getByText('19min 55% FTP')).toBeInTheDocument();
  });

  it('does not show a sync badge for intervals-origin planned workouts', async () => {
    mockedLoadEvent.mockResolvedValue(
      makeEvent({
        id: 51,
        startDateLocal: '2026-04-12',
        name: 'Opener Grojec',
        plannedSource: 'intervals',
        syncStatus: null,
        eventDefinition: makeEventDefinition({
          rawWorkoutDoc: '- 19min 55%',
          summary: makeWorkoutSummary({ totalDurationSeconds: 1140, estimatedTrainingStressScore: 16 }),
        }),
      }),
    );
    mockedLoadActivity.mockResolvedValue(undefined as never);

    render(
      <WorkoutDetailModal
        apiBaseUrl=""
        selection={makeSelection({
          dateKey: '2026-04-12',
          event: makeEvent({
            id: 51,
            startDateLocal: '2026-04-12',
            name: 'Opener Grojec',
            plannedSource: 'intervals',
            syncStatus: null,
            eventDefinition: makeEventDefinition({
              rawWorkoutDoc: '- 19min 55%',
              summary: makeWorkoutSummary({ totalDurationSeconds: 1140, estimatedTrainingStressScore: 16 }),
            }),
          }),
        })}
        onClose={vi.fn()}
      />,
    );

    await waitFor(() => expect(within(metricCard('Duration')).getByText('19m')).toBeInTheDocument());

    expect(screen.queryByText(/not synced/i)).not.toBeInTheDocument();
    expect(screen.queryByRole('button', { name: /sync to intervals/i })).not.toBeInTheDocument();
  });
});
