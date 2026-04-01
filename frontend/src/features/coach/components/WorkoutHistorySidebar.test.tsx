import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import '../../../i18n';
import type { CoachWorkoutListItem } from '../types';
import { WorkoutHistorySidebar } from './WorkoutHistorySidebar';

const itemFixture: CoachWorkoutListItem = {
  id: '101',
  source: 'event',
  startDateLocal: '2026-03-24T09:00:00',
  event: {
    id: 101,
    startDateLocal: '2026-03-24T09:00:00',
    name: 'Wild Snow',
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
        totalDurationSeconds: 3600,
        estimatedNormalizedPowerWatts: null,
        estimatedAveragePowerWatts: null,
        estimatedIntensityFactor: null,
        estimatedTrainingStressScore: null,
      },
    },
    actualWorkout: null,
  },
  activity: null,
  summary: null,
  hasSummary: false,
  hasConversation: false,
};

afterEach(() => {
  cleanup();
});

describe('WorkoutHistorySidebar', () => {
  it('renders workout items and emits selection', () => {
    const onSelectWorkout = vi.fn();

    render(
      <WorkoutHistorySidebar
        items={[itemFixture]}
        selectedWorkoutId={null}
        state="ready"
        error={null}
        weekLabel="Mar 24 - Mar 30"
        canGoToNewerWeek={false}
        onOlderWeek={() => undefined}
        onNewerWeek={() => undefined}
        onSelectWorkout={onSelectWorkout}
      />,
    );

    fireEvent.click(screen.getByRole('button', { name: /wild snow/i }));

    expect(onSelectWorkout).toHaveBeenCalledWith('101');
    expect(screen.getByText(/previous workouts/i)).toBeInTheDocument();
  });

  it('shows a week-empty state when no workouts are available', () => {
    render(
      <WorkoutHistorySidebar
        items={[]}
        selectedWorkoutId={null}
        state="ready"
        error={null}
        weekLabel="Mar 24 - Mar 30"
        canGoToNewerWeek={false}
        onOlderWeek={() => undefined}
        onNewerWeek={() => undefined}
        onSelectWorkout={() => undefined}
      />,
    );

    expect(screen.getByText(/no workouts found for this week/i)).toBeInTheDocument();
  });

  it('shows credentials guidance when intervals connection is required', () => {
    render(
      <WorkoutHistorySidebar
        items={[]}
        selectedWorkoutId={null}
        state="credentials-required"
        error={null}
        weekLabel="Mar 24 - Mar 30"
        canGoToNewerWeek={true}
        onOlderWeek={() => undefined}
        onNewerWeek={() => undefined}
        onSelectWorkout={() => undefined}
      />,
    );

    expect(screen.getByText(/connect intervals.icu/i)).toBeInTheDocument();
  });
});
