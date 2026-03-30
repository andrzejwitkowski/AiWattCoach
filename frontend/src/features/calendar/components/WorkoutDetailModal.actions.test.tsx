import { cleanup, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { makeActualWorkout, makeEvent, makeEventDefinition, makeSelection, makeWorkoutSummary } from '../testData';
import { mockedDownloadFit, mockedLoadActivity, mockedLoadEvent } from './WorkoutDetailModal.testHelpers';
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

    await waitFor(() => expect(screen.getByText('Race Prep')).toBeInTheDocument());

    await userEvent.click(screen.getByRole('button', { name: /download fit/i }));

    await waitFor(() => expect(mockedDownloadFit).toHaveBeenCalledWith('', 31));

    expect(createObjectURL).toHaveBeenCalledTimes(1);
    expect(click).toHaveBeenCalledTimes(1);
    expect(revokeObjectURL).toHaveBeenCalledWith('blob:fit-download');

    createElementSpy.mockRestore();
    URL.createObjectURL = originalCreateObjectURL;
    URL.revokeObjectURL = originalRevokeObjectURL;
  });
});
