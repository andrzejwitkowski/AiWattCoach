import { cleanup, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it, vi } from 'vitest';

import '../../../i18n';
import { makeActualWorkout, makeEvent, makeEventDefinition, makeSelection, makeWorkoutSummary } from '../testData';
import { HttpError } from '../../../lib/httpClient';
import { mockedDownloadFit, mockedLoadActivity, mockedLoadEvent, mockedSyncPlannedWorkout } from './WorkoutDetailModal.testHelpers';
import { WorkoutDetailModal } from './WorkoutDetailModal';

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

function dateKeyFromUtcOffset(days: number) {
  const now = new Date();
  const utcDate = new Date(Date.UTC(now.getUTCFullYear(), now.getUTCMonth(), now.getUTCDate()));
  utcDate.setUTCDate(utcDate.getUTCDate() + days);
  return utcDate.toISOString().slice(0, 10);
}

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
    const workoutDate = dateKeyFromUtcOffset(2);

    mockedLoadEvent.mockResolvedValue(
      makeEvent({
        id: 901,
        startDateLocal: workoutDate,
        name: 'Predicted Build',
        indoor: false,
        plannedSource: 'predicted',
        syncStatus: 'modified',
        projectedWorkout: {
          projectedWorkoutId: `training-plan:user-1:w1:1:${workoutDate}`,
          operationKey: 'training-plan:user-1:w1:1',
          date: workoutDate,
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
        startDateLocal: workoutDate,
        name: 'Predicted Build',
        indoor: false,
        plannedSource: 'predicted',
        syncStatus: 'synced',
        linkedIntervalsEventId: 91,
        projectedWorkout: {
          projectedWorkoutId: `training-plan:user-1:w1:1:${workoutDate}`,
          operationKey: 'training-plan:user-1:w1:1',
          date: workoutDate,
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
          dateKey: workoutDate,
          event: makeEvent({
            id: 901,
            startDateLocal: workoutDate,
            name: 'Predicted Build',
            indoor: false,
            plannedSource: 'predicted',
            syncStatus: 'modified',
            projectedWorkout: {
              projectedWorkoutId: `training-plan:user-1:w1:1:${workoutDate}`,
              operationKey: 'training-plan:user-1:w1:1',
              date: workoutDate,
              sourceWorkoutId: 'w1',
            },
          }),
        })}
        onClose={vi.fn()}
      />,
    );

    const syncButton = await screen.findByRole('button', { name: /sync to wahoo/i });

    await userEvent.click(syncButton);

    await waitFor(() => expect(mockedSyncPlannedWorkout).toHaveBeenCalledWith('', 'training-plan:user-1:w1:1', workoutDate));
    await waitFor(() => expect(screen.getByText(/synced/i)).toBeInTheDocument());
  });

  it('shows sync failure feedback when the planned workout sync request fails', async () => {
    const workoutDate = dateKeyFromUtcOffset(3);

    mockedLoadEvent.mockResolvedValue(
      makeEvent({
        id: 902,
        startDateLocal: workoutDate,
        name: 'Predicted Failure',
        indoor: false,
        plannedSource: 'predicted',
        syncStatus: 'modified',
        projectedWorkout: {
          projectedWorkoutId: `training-plan:user-1:w1:2:${workoutDate}`,
          operationKey: 'training-plan:user-1:w1:2',
          date: workoutDate,
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
          dateKey: workoutDate,
          event: makeEvent({
            id: 902,
            startDateLocal: workoutDate,
            name: 'Predicted Failure',
            indoor: false,
            plannedSource: 'predicted',
            syncStatus: 'modified',
            projectedWorkout: {
              projectedWorkoutId: `training-plan:user-1:w1:2:${workoutDate}`,
              operationKey: 'training-plan:user-1:w1:2',
              date: workoutDate,
              sourceWorkoutId: 'w2',
            },
          }),
        })}
        onClose={vi.fn()}
      />,
    );

    const syncButton = await screen.findByRole('button', { name: /sync to wahoo/i });

    await userEvent.click(syncButton);

    await waitFor(() => expect(screen.getByText(/unable to sync this planned workout to wahoo right now/i)).toBeInTheDocument());
  });

  it('shows the ftp warning when Wahoo sync requires cycling ftp settings', async () => {
    const workoutDate = dateKeyFromUtcOffset(3);

    mockedLoadEvent.mockResolvedValue(
      makeEvent({
        id: 904,
        startDateLocal: workoutDate,
        name: 'Missing FTP',
        indoor: false,
        plannedSource: 'predicted',
        syncStatus: 'modified',
        projectedWorkout: {
          projectedWorkoutId: `training-plan:user-1:w1:4:${workoutDate}`,
          operationKey: 'training-plan:user-1:w1:4',
          date: workoutDate,
          sourceWorkoutId: 'w4',
        },
        eventDefinition: makeEventDefinition({
          rawWorkoutDoc: '- 45min endurance',
          summary: makeWorkoutSummary({ totalDurationSeconds: 2700 }),
        }),
      }),
    );
    mockedLoadActivity.mockResolvedValue(undefined as never);
    mockedSyncPlannedWorkout.mockRejectedValue(
      new HttpError(400, 'Set your cycling FTP in Settings before syncing to Wahoo', {
        message: 'Set your cycling FTP in Settings before syncing to Wahoo',
      }),
    );

    render(
      <WorkoutDetailModal
        apiBaseUrl=""
        selection={makeSelection({
          dateKey: workoutDate,
          event: makeEvent({
            id: 904,
            startDateLocal: workoutDate,
            name: 'Missing FTP',
            indoor: false,
            plannedSource: 'predicted',
            syncStatus: 'modified',
            projectedWorkout: {
              projectedWorkoutId: `training-plan:user-1:w1:4:${workoutDate}`,
              operationKey: 'training-plan:user-1:w1:4',
              date: workoutDate,
              sourceWorkoutId: 'w4',
            },
          }),
        })}
        onClose={vi.fn()}
      />,
    );

    const syncButton = await screen.findByRole('button', { name: /sync to wahoo/i });

    await userEvent.click(syncButton);

    await waitFor(() => expect(screen.getByText(/set your cycling ftp in settings before syncing to wahoo/i)).toBeInTheDocument());
  });

  it('shows the Wahoo connection warning when sync returns 422', async () => {
    const workoutDate = dateKeyFromUtcOffset(3);

    mockedLoadEvent.mockResolvedValue(
      makeEvent({
        id: 906,
        startDateLocal: workoutDate,
        name: 'Connect Wahoo First',
        indoor: false,
        plannedSource: 'predicted',
        syncStatus: 'unsynced',
        projectedWorkout: {
          projectedWorkoutId: `training-plan:user-1:w1:6:${workoutDate}`,
          operationKey: 'training-plan:user-1:w1:6',
          date: workoutDate,
          sourceWorkoutId: 'w6',
        },
        eventDefinition: makeEventDefinition({
          rawWorkoutDoc: '- 45min endurance',
          summary: makeWorkoutSummary({ totalDurationSeconds: 2700 }),
        }),
      }),
    );
    mockedLoadActivity.mockResolvedValue(undefined as never);
    mockedSyncPlannedWorkout.mockRejectedValue(new HttpError(422, 'unprocessable entity'));

    render(
      <WorkoutDetailModal
        apiBaseUrl=""
        selection={makeSelection({
          dateKey: workoutDate,
          event: makeEvent({
            id: 906,
            startDateLocal: workoutDate,
            name: 'Connect Wahoo First',
            indoor: false,
            plannedSource: 'predicted',
            syncStatus: 'unsynced',
            projectedWorkout: {
              projectedWorkoutId: `training-plan:user-1:w1:6:${workoutDate}`,
              operationKey: 'training-plan:user-1:w1:6',
              date: workoutDate,
              sourceWorkoutId: 'w6',
            },
          }),
        })}
        onClose={vi.fn()}
      />,
    );

    const syncButton = await screen.findByRole('button', { name: /sync to wahoo/i });

    await userEvent.click(syncButton);

    await waitFor(() => expect(screen.getByText(/connect wahoo in settings before syncing this planned workout/i)).toBeInTheDocument());
  });

  it('disables Wahoo sync outside the allowed date window', async () => {
    const workoutDate = dateKeyFromUtcOffset(8);

    mockedLoadEvent.mockResolvedValue(
      makeEvent({
        id: 905,
        startDateLocal: workoutDate,
        name: 'Too Far Away',
        indoor: false,
        plannedSource: 'predicted',
        syncStatus: 'unsynced',
        projectedWorkout: {
          projectedWorkoutId: `training-plan:user-1:w1:5:${workoutDate}`,
          operationKey: 'training-plan:user-1:w1:5',
          date: workoutDate,
          sourceWorkoutId: 'w5',
        },
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
          dateKey: workoutDate,
          event: makeEvent({
            id: 905,
            startDateLocal: workoutDate,
            name: 'Too Far Away',
            indoor: false,
            plannedSource: 'predicted',
            syncStatus: 'unsynced',
            projectedWorkout: {
              projectedWorkoutId: `training-plan:user-1:w1:5:${workoutDate}`,
              operationKey: 'training-plan:user-1:w1:5',
              date: workoutDate,
              sourceWorkoutId: 'w5',
            },
          }),
        })}
        onClose={vi.fn()}
      />,
    );

    const syncButton = await screen.findByRole('button', { name: /sync to wahoo/i });

    expect(syncButton).toBeDisabled();
    expect(screen.getByText(/only workouts scheduled for today through the next 6 days can sync to wahoo/i)).toBeInTheDocument();
    expect(mockedSyncPlannedWorkout).not.toHaveBeenCalled();
  });

  it('does not show Wahoo sync for projected rest days', async () => {
    const workoutDate = dateKeyFromUtcOffset(2);

    mockedLoadEvent.mockResolvedValue(
      makeEvent({
        id: 907,
        startDateLocal: workoutDate,
        name: 'Rest Day',
        indoor: false,
        restDay: true,
        restDayReason: 'Need recovery before next block',
        plannedSource: 'predicted',
        syncStatus: 'unsynced',
        projectedWorkout: {
          projectedWorkoutId: `training-plan:user-1:w1:7:${workoutDate}`,
          operationKey: 'training-plan:user-1:w1:7',
          date: workoutDate,
          sourceWorkoutId: 'w7',
          restDay: true,
          restDayReason: 'Need recovery before next block',
        },
        eventDefinition: makeEventDefinition({
          summary: makeWorkoutSummary({ totalDurationSeconds: 0 }),
        }),
      }),
    );
    mockedLoadActivity.mockResolvedValue(undefined as never);

    render(
      <WorkoutDetailModal
        apiBaseUrl=""
        selection={makeSelection({
          dateKey: workoutDate,
          event: makeEvent({
            id: 907,
            startDateLocal: workoutDate,
            name: 'Rest Day',
            indoor: false,
            restDay: true,
            restDayReason: 'Need recovery before next block',
            plannedSource: 'predicted',
            syncStatus: 'unsynced',
            projectedWorkout: {
              projectedWorkoutId: `training-plan:user-1:w1:7:${workoutDate}`,
              operationKey: 'training-plan:user-1:w1:7',
              date: workoutDate,
              sourceWorkoutId: 'w7',
              restDay: true,
              restDayReason: 'Need recovery before next block',
            },
          }),
        })}
        onClose={vi.fn()}
      />,
    );

    await waitFor(() => expect(screen.getByText('Rest Day')).toBeInTheDocument());

    expect(screen.queryByRole('button', { name: /sync to wahoo/i })).not.toBeInTheDocument();
    expect(mockedSyncPlannedWorkout).not.toHaveBeenCalled();
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
            indoor: false,
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
            indoor: false,
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

  it('does not show Wahoo sync for indoor workouts', async () => {
    const workoutDate = dateKeyFromUtcOffset(2);

    mockedLoadEvent.mockResolvedValue(
      makeEvent({
        id: 908,
        startDateLocal: workoutDate,
        name: 'Indoor Trainer Session',
        indoor: true,
        plannedSource: 'predicted',
        syncStatus: 'modified',
        projectedWorkout: {
          projectedWorkoutId: `training-plan:user-1:w1:8:${workoutDate}`,
          operationKey: 'training-plan:user-1:w1:8',
          date: workoutDate,
          sourceWorkoutId: 'w8',
        },
        eventDefinition: makeEventDefinition({
          rawWorkoutDoc: '- 45min endurance',
          summary: makeWorkoutSummary({ totalDurationSeconds: 2700 }),
        }),
      }),
    );
    mockedLoadActivity.mockResolvedValue(undefined as never);

    render(
      <WorkoutDetailModal
        apiBaseUrl=""
        selection={makeSelection({
          dateKey: workoutDate,
          event: makeEvent({
            id: 908,
            startDateLocal: workoutDate,
            name: 'Indoor Trainer Session',
            indoor: true,
            plannedSource: 'predicted',
            syncStatus: 'modified',
            projectedWorkout: {
              projectedWorkoutId: `training-plan:user-1:w1:8:${workoutDate}`,
              operationKey: 'training-plan:user-1:w1:8',
              date: workoutDate,
              sourceWorkoutId: 'w8',
            },
          }),
        })}
        onClose={vi.fn()}
      />,
    );

    await waitFor(() => expect(screen.getByText('Indoor Trainer Session')).toBeInTheDocument());

    expect(screen.queryByRole('button', { name: /sync to wahoo/i })).not.toBeInTheDocument();
    expect(mockedSyncPlannedWorkout).not.toHaveBeenCalled();
  });
});
