import { render, screen, within } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';

import '../../../i18n';
import { makeActivity, makeCalendarDay, makeEvent } from '../testData';
import { CalendarDayCell } from './CalendarDayCell';

describe('CalendarDayCell content', () => {
  it('renders a rest day state when no data is present', () => {
    render(<CalendarDayCell day={makeCalendarDay({ date: new Date(2026, 2, 23), dateKey: '2026-03-23' })} isToday={false} />);

    expect(screen.getByText(/rest day/i)).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: /rest day/i })).not.toBeInTheDocument();
  });

  it('renders workout content when activity data exists', () => {
    const day = makeCalendarDay({
      date: new Date(2026, 2, 24),
      dateKey: '2026-03-24',
      activities: [
        makeActivity({
          id: 'a1',
          name: 'Morning Ride',
          distanceMeters: 40000,
          movingTimeSeconds: 5400,
          elapsedTimeSeconds: 5600,
          hasHeartRate: true,
          metrics: { trainingStressScore: 88, calories: 950 },
        }),
      ],
    });

    render(<CalendarDayCell day={day} isToday={true} />);

    expect(screen.getByText('Morning Ride')).toBeInTheDocument();
    expect(screen.getByText(/90 min/i)).toBeInTheDocument();
    expect(screen.getByText(/88 TSS/i)).toBeInTheDocument();
  });

  it('shows a more-items indicator when multiple items exist on one day', () => {
    const day = makeCalendarDay({
      date: new Date(2026, 2, 24),
      dateKey: '2026-03-24',
      events: [makeEvent({ id: 2, name: 'Planned ride', indoor: false })],
      activities: [
        makeActivity({
          id: 'a1',
          name: 'Morning Ride',
          distanceMeters: 40000,
          movingTimeSeconds: 5400,
          elapsedTimeSeconds: 5600,
          hasHeartRate: true,
          metrics: { trainingStressScore: 88, calories: 950 },
        }),
      ],
    });

    render(<CalendarDayCell day={day} isToday={false} />);

    expect(screen.getByText('+1 more item')).toBeInTheDocument();
  });

  it('does not label unnamed training activity as a rest day', () => {
    const day = makeCalendarDay({
      activities: [
        makeActivity({
          id: 'a2',
          name: null,
          distanceMeters: 32000,
          movingTimeSeconds: 3600,
          elapsedTimeSeconds: 3700,
          hasHeartRate: true,
          metrics: { trainingStressScore: 55, calories: 700 },
        }),
      ],
    });

    const { container } = render(<CalendarDayCell day={day} isToday={false} />);

    expect(screen.getByText('Ride')).toBeInTheDocument();
    expect(container).not.toHaveTextContent(/^Rest Day$/i);
  });

  it('uses the primary activity type as subtitle fallback when activity metrics are missing', () => {
    const day = makeCalendarDay({
      date: new Date(2026, 2, 27),
      dateKey: '2026-03-27',
      events: [makeEvent({ id: 4, name: 'Planned swim set', category: 'SWIM', indoor: false })],
      activities: [
        makeActivity({
          id: 'a4',
          name: 'Evening Ride',
          distanceMeters: 0,
          movingTimeSeconds: null,
          elapsedTimeSeconds: null,
          hasHeartRate: false,
        }),
      ],
    });

    const { container } = render(<CalendarDayCell day={day} isToday={false} />);

    expect(container).toHaveTextContent('Evening Ride');
    expect(container).toHaveTextContent('Ride');
    expect(container).not.toHaveTextContent('Swim');
  });

  it('does not leak planned summary metrics into a mixed day activity subtitle', () => {
    const day = makeCalendarDay({
      date: new Date(2026, 2, 27),
      dateKey: '2026-03-27',
      events: [
        makeEvent({
          id: 40,
          name: 'Planned Build',
          eventDefinition: {
            summary: {
              totalDurationSeconds: 3600,
              estimatedTrainingStressScore: 64.4,
            },
          },
        }),
      ],
      activities: [
        makeActivity({
          id: 'a40',
          name: 'Evening Ride',
          movingTimeSeconds: null,
          elapsedTimeSeconds: null,
          metrics: { trainingStressScore: null },
          hasHeartRate: false,
        }),
      ],
    });

    const { container } = render(<CalendarDayCell day={day} isToday={false} />);

    expect(container).toHaveTextContent('Evening Ride');
    expect(container).toHaveTextContent('Ride');
    expect(container).not.toHaveTextContent('64.4 TSS');
    expect(container).not.toHaveTextContent('60 min');
  });

  it('shows planned workout summary details and coach label for planned-only days', () => {
    const day = makeCalendarDay({
      date: new Date(2026, 2, 27),
      dateKey: '2026-03-27',
      events: [
        makeEvent({
          id: 5,
          name: 'Coach Build',
          eventDefinition: {
            summary: {
              totalDurationSeconds: 3600,
              estimatedTrainingStressScore: 64,
            },
          },
        }),
      ],
    });

    const { container } = render(<CalendarDayCell day={day} isToday={false} />);
    const dayCell = container.firstElementChild as HTMLElement;

    expect(within(dayCell).getByText('Planned Workout')).toBeInTheDocument();
    expect(within(dayCell).getByText('Coach Build')).toBeInTheDocument();
    expect(within(dayCell).getByText('60 min • 64 TSS')).toBeInTheDocument();
  });

  it('shows modified label for predicted workouts with pending schedule changes', () => {
    const day = makeCalendarDay({
      date: new Date(2026, 2, 27),
      dateKey: '2026-03-27',
      events: [
        makeEvent({
          id: 905,
          name: 'Coach Build',
          plannedSource: 'predicted',
          syncStatus: 'modified',
          projectedWorkout: {
            projectedWorkoutId: 'training-plan:user-1:w1:1:2026-03-27',
            operationKey: 'training-plan:user-1:w1:1',
            date: '2026-03-27',
            sourceWorkoutId: 'w1',
          },
          eventDefinition: {
            summary: {
              totalDurationSeconds: 3600,
              estimatedTrainingStressScore: 64,
            },
          },
        }),
      ],
    });

    const { container } = render(<CalendarDayCell day={day} isToday={false} />);
    const dayCell = container.firstElementChild as HTMLElement;

    expect(within(dayCell).getByText('Modified')).toBeInTheDocument();
    expect(dayCell.className).toContain('border-[#b9b082]/50');
    expect(within(dayCell).getByTestId('planned-sync-status')).toHaveAttribute('aria-label', 'Not Synced');
  });

  it('shows a disconnected sync indicator for unsynced planned workouts', () => {
    const day = makeCalendarDay({
      date: new Date(2026, 3, 11),
      dateKey: '2026-04-11',
      events: [
        makeEvent({
          id: 906,
          name: 'Active Recovery',
          plannedSource: 'predicted',
          syncStatus: 'unsynced',
          projectedWorkout: {
            projectedWorkoutId: 'training-plan:user-1:w1:1:2026-04-11',
            operationKey: 'training-plan:user-1:w1:1',
            date: '2026-04-11',
            sourceWorkoutId: 'w1',
          },
          eventDefinition: {
            summary: {
              totalDurationSeconds: 2700,
              estimatedTrainingStressScore: 19,
            },
          },
        }),
      ],
    });

    const { container } = render(<CalendarDayCell day={day} isToday={false} />);
    const dayCell = container.firstElementChild as HTMLElement;

    expect(dayCell.className).toContain('border-[#b9b082]/50');
    expect(within(dayCell).getByText('Not Synced')).toBeInTheDocument();
    expect(within(dayCell).getByTestId('planned-sync-status')).toHaveAttribute('aria-label', 'Not Synced');
  });

  it('shows a connected sync indicator for synced planned workouts', () => {
    const day = makeCalendarDay({
      date: new Date(2026, 3, 16),
      dateKey: '2026-04-16',
      events: [
        makeEvent({
          id: 907,
          name: 'Priming Session',
          plannedSource: 'predicted',
          syncStatus: 'synced',
          linkedIntervalsEventId: 42,
          projectedWorkout: {
            projectedWorkoutId: 'training-plan:user-1:w1:1:2026-04-16',
            operationKey: 'training-plan:user-1:w1:1',
            date: '2026-04-16',
            sourceWorkoutId: 'w1',
          },
          eventDefinition: {
            summary: {
              totalDurationSeconds: 1680,
              estimatedTrainingStressScore: 17,
            },
          },
        }),
      ],
    });

    const { container } = render(<CalendarDayCell day={day} isToday={false} />);
    const dayCell = container.firstElementChild as HTMLElement;

    expect(dayCell.className).toContain('border-[#80d998]/55');
    expect(within(dayCell).getByText('Synced')).toBeInTheDocument();
    expect(within(dayCell).getByTestId('planned-sync-status')).toHaveAttribute('aria-label', 'Synced');
  });

  it('does not show sync visuals for non-predicted planned workouts', () => {
    const day = makeCalendarDay({
      date: new Date(2026, 3, 16),
      dateKey: '2026-04-16',
      events: [
        makeEvent({
          id: 908,
          name: 'Intervals Planned Session',
          plannedSource: 'intervals',
          syncStatus: 'synced',
          linkedIntervalsEventId: 43,
          eventDefinition: {
            summary: {
              totalDurationSeconds: 2400,
              estimatedTrainingStressScore: 32,
            },
          },
        }),
      ],
    });

    const { container } = render(<CalendarDayCell day={day} isToday={false} />);
    const dayCell = container.firstElementChild as HTMLElement;

    expect(dayCell.className).not.toContain('border-[#80d998]/55');
    expect(dayCell.className).not.toContain('border-[#b9b082]/50');
    expect(within(dayCell).queryByText('Synced')).not.toBeInTheDocument();
    expect(within(dayCell).queryByText('Not Synced')).not.toBeInTheDocument();
    expect(within(dayCell).queryByTestId('planned-sync-status')).not.toBeInTheDocument();
  });

  it('does not show the coach planned badge for non-workout events', () => {
    const day = makeCalendarDay({
      events: [
        makeEvent({
          id: 6,
          name: 'Club Race',
          category: 'RACE',
          eventDefinition: {
            intervals: [],
            segments: [],
            rawWorkoutDoc: null,
            summary: {
              totalDurationSeconds: 0,
              estimatedTrainingStressScore: null,
            },
          },
        }),
      ],
    });

    const { container } = render(<CalendarDayCell day={day} isToday={false} />);
    const dayCell = container.firstElementChild as HTMLElement;

    expect(within(dayCell).queryByText('Planned Workout')).not.toBeInTheDocument();
    expect(within(dayCell).getByText('Club Race')).toBeInTheDocument();
  });

  it('does not show the planned workout badge for completed events without loaded activities', () => {
    const day = makeCalendarDay({
      events: [
        makeEvent({
          id: 41,
          name: 'Completed Build',
          actualWorkout: {
            activityId: 'a41',
            activityName: 'Completed Build Outside',
          },
          eventDefinition: {
            intervals: [
              {
                definition: '20min tempo',
                repeatCount: 1,
                durationSeconds: 1200,
                targetPercentFtp: 90,
                zoneId: 3,
              },
            ],
            summary: {
              totalDurationSeconds: 1200,
              estimatedTrainingStressScore: 30,
            },
          },
        }),
      ],
      activities: [],
    });

    const { container } = render(<CalendarDayCell day={day} isToday={false} />);
    const dayCell = container.firstElementChild as HTMLElement;

    expect(within(dayCell).queryByText('Planned Workout')).not.toBeInTheDocument();
    expect(within(dayCell).getByText('Completed Build')).toBeInTheDocument();
    expect(dayCell).not.toHaveTextContent('20 min');
    expect(dayCell).not.toHaveTextContent('30 TSS');
  });

  it('does not render a clickable button without a real select handler', () => {
    const day = makeCalendarDay({
      date: new Date(2026, 2, 29),
      dateKey: '2026-03-29',
      events: [makeEvent({ id: 7, name: 'Planned ride', indoor: false })],
    });

    const { container } = render(<CalendarDayCell day={day} isToday={false} />);
    const dayCell = container.firstElementChild as HTMLElement;

    expect(within(dayCell).queryByRole('button')).not.toBeInTheDocument();
    expect(within(dayCell).getByText('Planned ride')).toBeInTheDocument();
  });

  it('renders a clickable button when a select handler is provided', () => {
    const day = makeCalendarDay({
      events: [makeEvent({ id: 8, name: 'Selectable ride' })],
    });
    const onSelect = vi.fn();

    render(<CalendarDayCell day={day} isToday={false} onSelect={onSelect} />);

    expect(screen.getByRole('button', { name: /selectable ride/i })).toBeInTheDocument();
  });
});
