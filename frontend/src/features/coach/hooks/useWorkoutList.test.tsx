import { renderHook, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import type { IntervalEvent } from '../../intervals/types';
import { listEvents } from '../../intervals/api/intervals';
import { HttpError } from '../../../lib/httpClient';
import { listWorkoutSummaries } from '../api/workoutSummary';
import { useWorkoutList } from './useWorkoutList';

vi.mock('../../intervals/api/intervals', () => ({
  listEvents: vi.fn(),
}));

vi.mock('../api/workoutSummary', () => ({
  listWorkoutSummaries: vi.fn(),
}));

const eventFixture: IntervalEvent = {
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
};

afterEach(() => {
  vi.clearAllMocks();
});

describe('useWorkoutList', () => {
  it('loads up to seven workouts and merges summary status', async () => {
    vi.mocked(listEvents).mockResolvedValue(
      Array.from({ length: 9 }, (_, index) => ({
        ...eventFixture,
        id: 101 + index,
        startDateLocal: `2026-03-${String(24 - index).padStart(2, '0')}T09:00:00`,
      })),
    );
    vi.mocked(listWorkoutSummaries).mockResolvedValue([
      {
        id: 'summary-1',
        eventId: '101',
        rpe: 6,
        messages: [
          {
            id: 'message-1',
            role: 'coach',
            content: 'Nice work.',
            createdAtEpochSeconds: 1,
          },
        ],
        createdAtEpochSeconds: 1,
        updatedAtEpochSeconds: 2,
      },
    ]);

    const { result } = renderHook(() => useWorkoutList({ apiBaseUrl: '' }));

    await waitFor(() => {
      expect(result.current.state).toBe('ready');
    });

    expect(result.current.items).toHaveLength(7);
    expect(result.current.items[0]?.hasConversation).toBe(true);
  });

  it('keeps unknown intervals event categories in the workout list', async () => {
    vi.mocked(listEvents).mockResolvedValue([
      {
        ...eventFixture,
        id: 301,
        name: 'March 31 Ride',
        startDateLocal: '2026-03-31T09:00:00',
        category: 'OTHER',
      },
    ]);
    vi.mocked(listWorkoutSummaries).mockResolvedValue([]);

    const { result } = renderHook(() => useWorkoutList({ apiBaseUrl: '' }));

    await waitFor(() => {
      expect(result.current.state).toBe('ready');
    });

    expect(result.current.items).toHaveLength(1);
    expect(result.current.items[0]?.event.id).toBe(301);
  });

  it('pages through older workouts from a larger recent history window', async () => {
    vi.mocked(listEvents).mockResolvedValue(
      Array.from({ length: 10 }, (_, index) => ({
        ...eventFixture,
        id: 200 + index,
        name: `Workout ${index + 1}`,
        startDateLocal: `2026-03-${String(28 - index).padStart(2, '0')}T09:00:00`,
      })),
    );
    vi.mocked(listWorkoutSummaries).mockResolvedValue([]);

    const { result } = renderHook(() => useWorkoutList({ apiBaseUrl: '' }));

    await waitFor(() => {
      expect(result.current.state).toBe('ready');
    });

    expect(result.current.items).toHaveLength(7);
    expect(result.current.items[0]?.event.name).toBe('Workout 1');

    result.current.goToOlderWeek();

    await waitFor(() => {
      expect(result.current.items).toHaveLength(3);
    });

    expect(result.current.items[0]?.event.name).toBe('Workout 8');
    expect(result.current.canGoToNewerWeek).toBe(true);
  });

  it('marks missing intervals credentials as a dedicated state', async () => {
    vi.mocked(listEvents).mockRejectedValue(new HttpError(422, 'bad request'));

    const { result } = renderHook(() => useWorkoutList({ apiBaseUrl: '' }));

    await waitFor(() => {
      expect(result.current.state).toBe('credentials-required');
    });
  });
});
