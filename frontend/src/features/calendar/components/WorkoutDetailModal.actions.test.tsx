import { cleanup, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { makeActualWorkout, makeEvent, makeEventDefinition, makeSelection, makeWorkoutSummary } from '../testData';
import { HttpError } from '../../../lib/httpClient';
import { mockedDownloadFit, mockedLoadActivity, mockedLoadEvent, mockedSyncPlannedWorkout } from './WorkoutDetailModal.testHelpers';
import { WorkoutDetailModal } from './WorkoutDetailModal';

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

describe('WorkoutDetailModal actions', () => {
  it('hides FIT download action in completed mode', async () => {
    mockedLoadEvent.mockResolvedValue(
      makeEvent({
        id: 23,
        startDateLocal: '2026-03-26',
        name: 'Completed Workout',
        indoor: false,
        eventDefinition: makeEventDefinition({ summary: makeWorkoutSummary() }),
        actualWorkout: makeActualWorkout({
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
        }),
      }),
    );
    mockedLoadActivity.mockResolvedValue(undefined as never);

    render(
      <WorkoutDetailModal
        apiBaseUrl=""
        selection={makeSelection({ dateKey: '2026-03-26', event: makeEvent({ id: 23, startDateLocal: '2026-03-26', name: 'Completed Workout', indoor: false }) })}
        onClose={vi.fn()}
      />,
    );

    await waitFor(() => expect(screen.getByText(/completed workout/i)).toBeInTheDocument());

    expect(screen.queryByRole('button', { name: /download fit/i })).not.toBeInTheDocument();
  });

  it('downloads the event FIT file from the modal action', async () => {
    mockedLoadEvent.mockResolvedValue(
      makeEvent({
        id: 31,
        startDateLocal: '2026-03-26',
        name: 'Race Prep',
        eventDefinition: makeEventDefinition({
          rawWorkoutDoc: '- 60min endurance',
          summary: makeWorkoutSummary({ totalDurationSeconds: 3600 }),
        }),
      }),
    );
    mockedLoadActivity.mockResolvedValue(undefined as never);
    mockedDownloadFit.mockResolvedValue(new Uint8Array([1, 2, 3]));

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

    render(
      <WorkoutDetailModal
        apiBaseUrl=""
        selection={makeSelection({ dateKey: '2026-03-26', event: makeEvent({ id: 31, startDateLocal: '2026-03-26', name: 'Race Prep' }) })}
        onClose={vi.fn()}
      />,
    );

    const downloadButton = await screen.findByRole('button', { name: /download fit/i });

    await userEvent.click(downloadButton);

    await waitFor(() => expect(mockedDownloadFit).toHaveBeenCalledWith('', 31));

    expect(createObjectURL).toHaveBeenCalledTimes(1);
    expect(click).toHaveBeenCalledTimes(1);

    await waitFor(() => expect(revokeObjectURL).toHaveBeenCalledWith('blob:fit-download'));

    createElementSpy.mockRestore();
    URL.createObjectURL = originalCreateObjectURL;
    URL.revokeObjectURL = originalRevokeObjectURL;
  });

  it('syncs a planned workout from the modal action', async () => {
    mockedLoadEvent.mockResolvedValue(
      makeEvent({
        id: 901,
        startDateLocal: '2026-03-26',
        name: 'Predicted Build',
        plannedSource: 'predicted',
        syncStatus: 'modified',
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
    );
    mockedLoadActivity.mockResolvedValue(undefined as never);
    mockedSyncPlannedWorkout.mockResolvedValue(
      makeEvent({
        id: 91,
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
    );

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
            syncStatus: 'modified',
            projectedWorkout: {
              projectedWorkoutId: 'training-plan:user-1:w1:1:2026-03-26',
              operationKey: 'training-plan:user-1:w1:1',
              date: '2026-03-26',
              sourceWorkoutId: 'w1',
            },
          }),
        })}
        onClose={vi.fn()}
      />,
    );

    const syncButton = await screen.findByRole('button', { name: /sync to intervals/i });

    await userEvent.click(syncButton);

    await waitFor(() => expect(mockedSyncPlannedWorkout).toHaveBeenCalledWith('', 'training-plan:user-1:w1:1', '2026-03-26'));
    await waitFor(() => expect(screen.getByText(/synced/i)).toBeInTheDocument());
  });

  it('shows sync failure feedback when the planned workout sync request fails', async () => {
    mockedLoadEvent.mockResolvedValue(
      makeEvent({
        id: 902,
        startDateLocal: '2026-03-27',
        name: 'Predicted Failure',
        plannedSource: 'predicted',
        syncStatus: 'modified',
        projectedWorkout: {
          projectedWorkoutId: 'training-plan:user-1:w1:2:2026-03-27',
          operationKey: 'training-plan:user-1:w1:2',
          date: '2026-03-27',
          sourceWorkoutId: 'w2',
        },
        eventDefinition: makeEventDefinition({
          rawWorkoutDoc: '- 45min endurance',
          summary: makeWorkoutSummary({ totalDurationSeconds: 2700 }),
        }),
      }),
    );
    mockedLoadActivity.mockResolvedValue(undefined as never);
    mockedSyncPlannedWorkout.mockRejectedValue(new HttpError(502, 'bad gateway'));

    render(
      <WorkoutDetailModal
        apiBaseUrl=""
        selection={makeSelection({
          dateKey: '2026-03-27',
          event: makeEvent({
            id: 902,
            startDateLocal: '2026-03-27',
            name: 'Predicted Failure',
            plannedSource: 'predicted',
            syncStatus: 'modified',
            projectedWorkout: {
              projectedWorkoutId: 'training-plan:user-1:w1:2:2026-03-27',
              operationKey: 'training-plan:user-1:w1:2',
              date: '2026-03-27',
              sourceWorkoutId: 'w2',
            },
          }),
        })}
        onClose={vi.fn()}
      />,
    );

    const syncButton = await screen.findByRole('button', { name: /sync to intervals/i });

    await userEvent.click(syncButton);

    await waitFor(() => expect(screen.getByText(/unable to sync this planned workout to intervals right now/i)).toBeInTheDocument());
  });

  it('does not request event details for unsynced predicted workouts', async () => {
    mockedLoadActivity.mockResolvedValue(undefined as never);

    render(
      <WorkoutDetailModal
        apiBaseUrl=""
        selection={makeSelection({
          dateKey: '2026-03-28',
          event: makeEvent({
            id: 903,
            startDateLocal: '2026-03-28',
            name: 'Unsynced Prediction',
            plannedSource: 'predicted',
            syncStatus: 'unsynced',
            projectedWorkout: {
              projectedWorkoutId: 'training-plan:user-1:w1:3:2026-03-28',
              operationKey: 'training-plan:user-1:w1:3',
              date: '2026-03-28',
              sourceWorkoutId: 'w3',
            },
          }),
        })}
        onClose={vi.fn()}
      />,
    );

    await waitFor(() => expect(screen.getByText('Unsynced Prediction')).toBeInTheDocument());

    expect(mockedLoadEvent).not.toHaveBeenCalled();
  });

  it('hides FIT download for unsynced predicted workouts with synthetic ids', async () => {
    mockedLoadActivity.mockResolvedValue(undefined as never);

    render(
      <WorkoutDetailModal
        apiBaseUrl=""
        selection={makeSelection({
          dateKey: '2026-03-28',
          event: makeEvent({
            id: 903,
            startDateLocal: '2026-03-28',
            name: 'Unsynced Prediction',
            plannedSource: 'predicted',
            syncStatus: 'unsynced',
            linkedIntervalsEventId: null,
            projectedWorkout: {
              projectedWorkoutId: 'training-plan:user-1:w1:3:2026-03-28',
              operationKey: 'training-plan:user-1:w1:3',
              date: '2026-03-28',
              sourceWorkoutId: 'w3',
            },
          }),
        })}
        onClose={vi.fn()}
      />,
    );

    await waitFor(() => expect(screen.getByText('Unsynced Prediction')).toBeInTheDocument());

    expect(screen.queryByRole('button', { name: /download fit/i })).not.toBeInTheDocument();
  });
});
