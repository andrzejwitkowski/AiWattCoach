import { act, renderHook, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import type { IntervalEvent } from '../../intervals/types';
import { listActivities, listEvents } from '../../intervals/api/intervals';
import { HttpError } from '../../../lib/httpClient';
import { listWorkoutSummaries } from '../api/workoutSummary';
import { useWorkoutList } from './useWorkoutList';

function formatWeekLabel(start: Date, end: Date): string {
  const formatter = new Intl.DateTimeFormat(undefined, {
    month: 'short',
    day: 'numeric',
  });

  return `${formatter.format(start)} - ${formatter.format(end)}`;
}

vi.mock('../../intervals/api/intervals', () => ({
  listActivities: vi.fn(),
  listEvents: vi.fn(),
}));

vi.mock('../api/workoutSummary', () => ({
  listWorkoutSummaries: vi.fn(),
}));

afterEach(() => {
  vi.clearAllMocks();
});

const eventFixture: IntervalEvent = {
  id: 101,
  startDateLocal: '2026-03-24T09:00:00',
  name: 'Wild Snow',
  category: 'WORKOUT',
  description: null,
  restDay: false,
  restDayReason: null,
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

const activityFixture = {
  id: 'activity-1',
  startDateLocal: '2026-03-24T09:00:00',
  startDate: '2026-03-24T08:00:00Z',
  name: 'Wild Snow',
  description: null,
  activityType: 'Ride',
  source: null,
  externalId: null,
  deviceName: null,
  distanceMeters: null,
  movingTimeSeconds: 3500,
  elapsedTimeSeconds: 3600,
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
};

afterEach(() => {
  vi.clearAllMocks();
});

describe('useWorkoutList', () => {
  it('loads up to seven workouts and merges summary status', async () => {
    const pastEvents = Array.from({ length: 9 }, (_, index) => ({
      ...eventFixture,
      id: 101 + index,
      startDateLocal: `2026-03-${String(24 - index).padStart(2, '0')}T09:00:00`,
    }));
    const matchingActivities = pastEvents.map((event) => ({
      ...activityFixture,
      id: `activity-${event.id}`,
      name: event.name,
      startDateLocal: event.startDateLocal,
      startDate: event.startDateLocal.replace('T09:00:00', 'T08:00:00Z'),
    }));
    vi.mocked(listActivities).mockResolvedValue(matchingActivities);
    vi.mocked(listEvents).mockResolvedValue(pastEvents);
    vi.mocked(listWorkoutSummaries).mockResolvedValue([
      {
        id: 'summary-1',
        workoutId: 'activity-101',
        rpe: 6,
        messages: [
          {
            id: 'message-1',
            role: 'coach',
            content: 'Nice work.',
            createdAtEpochSeconds: 1,
          },
        ],
        savedAtEpochSeconds: null,
        createdAtEpochSeconds: 1,
        updatedAtEpochSeconds: 2,
      },
    ]);

    const { result } = renderHook(() => useWorkoutList({ apiBaseUrl: '' }));

    await waitFor(() => {
      expect(result.current.state).toBe('ready');
    });

    expect(result.current.weekLabel).toBe(formatWeekLabel(new Date(2026, 2, 23), new Date(2026, 2, 29)));
    expect(result.current.items).toHaveLength(2);
    expect(result.current.items[0]?.hasConversation).toBe(true);
  });

  it('keeps activities whose matched event has an unknown category', async () => {
    vi.mocked(listActivities).mockResolvedValue([
      {
        ...activityFixture,
        id: 'activity-301',
        name: 'March 31 Ride',
        startDateLocal: '2026-03-31T09:00:00',
        startDate: '2026-03-31T08:00:00Z',
      },
    ]);
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

    expect(result.current.weekLabel).toBe(formatWeekLabel(new Date(2026, 2, 30), new Date(2026, 3, 5)));
    expect(result.current.items).toHaveLength(1);
    expect(result.current.items[0]?.source).toBe('activity');
    expect(result.current.items[0]?.activity?.id).toBe('activity-301');
    expect(result.current.items[0]?.event?.id).toBe(301);
  });

  it('pages through older workouts from a larger recent history window', async () => {
    const pastEvents = Array.from({ length: 10 }, (_, index) => ({
      ...eventFixture,
      id: 200 + index,
      name: `Workout ${index + 1}`,
      startDateLocal: `2026-03-${String(28 - index).padStart(2, '0')}T09:00:00`,
    }));
    const matchingActivities = pastEvents.map((event) => ({
      ...activityFixture,
      id: `activity-${event.id}`,
      name: event.name,
      startDateLocal: event.startDateLocal,
      startDate: event.startDateLocal.replace('T09:00:00', 'T08:00:00Z'),
    }));
    vi.mocked(listActivities).mockResolvedValue(matchingActivities);
    vi.mocked(listEvents).mockResolvedValue(pastEvents);
    vi.mocked(listWorkoutSummaries).mockResolvedValue([]);

    const { result } = renderHook(() => useWorkoutList({ apiBaseUrl: '' }));

    await waitFor(() => {
      expect(result.current.state).toBe('ready');
    });

    expect(result.current.weekLabel).toBe(formatWeekLabel(new Date(2026, 2, 23), new Date(2026, 2, 29)));
    expect(result.current.items).toHaveLength(6);

    act(() => {
      result.current.goToOlderWeek();
    });

    await waitFor(() => {
      expect(result.current.items).toHaveLength(4);
    });

    expect(result.current.weekLabel).toBe(formatWeekLabel(new Date(2026, 2, 16), new Date(2026, 2, 22)));
    expect(result.current.items[0]?.event?.name).toBe('Workout 7');
    expect(result.current.canGoToNewerWeek).toBe(true);
  });

  it('matches hinted activities to their event without leaving duplicates behind', async () => {
    vi.mocked(listEvents).mockResolvedValue([
      {
        ...eventFixture,
        id: 777,
        name: 'Hinted Workout',
        startDateLocal: '2026-03-24T09:00:00',
      },
    ]);
    vi.mocked(listActivities).mockResolvedValue([
      {
        ...activityFixture,
        id: 'activity-777',
        name: 'Completed Ride',
        description: 'paired_event_id=777',
        startDateLocal: '2026-03-24T10:00:00',
      },
    ]);
    vi.mocked(listWorkoutSummaries).mockResolvedValue([]);

    const { result } = renderHook(() => useWorkoutList({ apiBaseUrl: '' }));

    await waitFor(() => {
      expect(result.current.state).toBe('ready');
    });

    expect(result.current.items[0]?.source).toBe('activity');
    expect(result.current.items[0]?.event?.id).toBe(777);
  });

  it('prefers actualWorkout activity links over heuristic matching and keeps one item', async () => {
    vi.mocked(listEvents).mockResolvedValue([
      {
        ...eventFixture,
        id: 880,
        name: 'Planned Threshold Session',
        startDateLocal: '2026-03-24T09:00:00',
        actualWorkout: {
          activityId: 'activity-linked',
          activityName: 'Completed Threshold Ride',
          startDateLocal: '2026-03-24T10:00:00',
          powerValues: [],
          cadenceValues: [],
          heartRateValues: [],
          speedValues: [],
          averagePowerWatts: 210,
          normalizedPowerWatts: 225,
          trainingStressScore: 68,
          intensityFactor: 0.81,
          complianceScore: 0.92,
          matchedIntervals: [],
        },
      },
      {
        ...eventFixture,
        id: 881,
        name: 'Completed Threshold Ride',
        startDateLocal: '2026-03-24T10:00:00',
        actualWorkout: null,
      },
    ]);
    vi.mocked(listActivities).mockResolvedValue([
      {
        ...activityFixture,
        id: 'activity-linked',
        name: 'Completed Threshold Ride',
        startDateLocal: '2026-03-24T10:00:00',
        startDate: '2026-03-24T09:00:00Z',
      },
    ]);
    vi.mocked(listWorkoutSummaries).mockResolvedValue([]);

    const { result } = renderHook(() => useWorkoutList({ apiBaseUrl: '' }));

    await waitFor(() => {
      expect(result.current.state).toBe('ready');
    });

    expect(result.current.items).toHaveLength(1);
    expect(result.current.items[0]?.source).toBe('activity');
    expect(result.current.items[0]?.activity?.id).toBe('activity-linked');
    expect(result.current.items[0]?.event?.id).toBe(880);
    expect(result.current.items[0]?.event?.actualWorkout?.activityId).toBe('activity-linked');
  });

  it('keeps only the newest refresh result when loads overlap', async () => {
    let resolveFirst: (() => void) | undefined;
    let resolveSecond: (() => void) | undefined;

    vi.mocked(listWorkoutSummaries).mockResolvedValue([]);
    vi.mocked(listActivities)
      .mockImplementationOnce(() => new Promise((resolve) => {
        resolveFirst = () => resolve([
          {
            ...activityFixture,
            id: 'activity-901',
            name: 'Older result',
            startDateLocal: '2026-04-07T09:00:00',
            startDate: '2026-04-07T08:00:00Z',
          },
        ]);
      }))
      .mockImplementationOnce(() => new Promise((resolve) => {
        resolveSecond = () => resolve([
          {
            ...activityFixture,
            id: 'activity-900',
            name: 'Newer result',
            startDateLocal: '2026-04-07T09:00:00',
            startDate: '2026-04-07T08:00:00Z',
          },
        ]);
      }));
    vi.mocked(listEvents).mockResolvedValue([]);

    const { result } = renderHook(() => useWorkoutList({ apiBaseUrl: '' }));

    await act(async () => {
      const secondRefresh = result.current.refresh();
      resolveSecond?.();
      await secondRefresh;
    });

    await act(async () => {
      resolveFirst?.();
      await Promise.resolve();
    });

    await waitFor(() => {
      expect(result.current.state).toBe('ready');
    });

    await waitFor(() => {
      expect(result.current.items[0]?.activity?.name).toBe('Newer result');
    });
  });

  it('updates the matching item when a summary changes', async () => {
    vi.mocked(listActivities).mockResolvedValue([
      {
        ...activityFixture,
        id: 'activity-101',
        name: 'Wild Snow',
        startDateLocal: '2026-03-24T09:00:00',
        startDate: '2026-03-24T08:00:00Z',
      },
    ]);
    vi.mocked(listEvents).mockResolvedValue([
      {
        ...eventFixture,
        id: 101,
        startDateLocal: '2026-03-24T09:00:00',
      },
    ]);
    vi.mocked(listWorkoutSummaries).mockResolvedValue([]);

    const { result } = renderHook(() => useWorkoutList({ apiBaseUrl: '' }));

    await waitFor(() => {
      expect(result.current.state).toBe('ready');
    });

    act(() => {
      result.current.replaceSummary({
        id: 'summary-101',
        workoutId: 'activity-101',
        rpe: 7,
        messages: [
          {
            id: 'message-1',
            role: 'coach',
            content: 'Great work.',
            createdAtEpochSeconds: 1,
          },
        ],
        savedAtEpochSeconds: 2,
        createdAtEpochSeconds: 1,
        updatedAtEpochSeconds: 2,
      });
    });

    expect(result.current.items[0]?.hasSummary).toBe(true);
    expect(result.current.items[0]?.hasConversation).toBe(true);
    expect(result.current.items[0]?.summary?.savedAtEpochSeconds).toBe(2);
  });

  it('prefers activities and falls back to related event summaries when available', async () => {
    vi.mocked(listEvents).mockResolvedValue([
      {
        ...eventFixture,
        id: 451,
        name: 'Threshold Ride',
        startDateLocal: '2026-03-24T09:00:00',
      },
    ]);
    vi.mocked(listActivities).mockResolvedValue([
      {
        ...activityFixture,
        id: 'activity-451',
        name: 'Threshold Ride',
        startDateLocal: '2026-03-24T09:02:00',
        startDate: '2026-03-24T08:02:00Z',
      },
    ]);
    vi.mocked(listWorkoutSummaries).mockResolvedValue([
      {
        id: 'summary-451',
        workoutId: '451',
        rpe: 5,
        messages: [],
        savedAtEpochSeconds: null,
        createdAtEpochSeconds: 1,
        updatedAtEpochSeconds: 3,
      },
    ]);

    const { result } = renderHook(() => useWorkoutList({ apiBaseUrl: '' }));

    await waitFor(() => {
      expect(result.current.state).toBe('ready');
    });

    expect(result.current.items[0]?.source).toBe('activity');
    expect(result.current.items[0]?.activity?.id).toBe('activity-451');
    expect(result.current.items[0]?.event?.id).toBe(451);
    expect(result.current.items[0]?.summary).toBeNull();
    expect(result.current.items[0]?.id).toBe('activity-451');
  });

  it('keeps activity identity when a legacy event summary also exists', async () => {
    vi.mocked(listEvents).mockResolvedValue([
      {
        ...eventFixture,
        id: 990,
        name: 'Linked Planned Workout',
        startDateLocal: '2026-03-24T09:00:00',
        actualWorkout: {
          activityId: 'activity-990',
          activityName: 'Completed Ride',
          startDateLocal: '2026-03-24T10:00:00',
          powerValues: [],
          cadenceValues: [],
          heartRateValues: [],
          speedValues: [],
          averagePowerWatts: 200,
          normalizedPowerWatts: 215,
          trainingStressScore: 60,
          intensityFactor: 0.8,
          complianceScore: 0.9,
          matchedIntervals: [],
        },
      },
    ]);
    vi.mocked(listActivities).mockResolvedValue([
      {
        ...activityFixture,
        id: 'activity-990',
        name: 'Completed Ride',
        startDateLocal: '2026-03-24T10:00:00',
        startDate: '2026-03-24T09:00:00Z',
      },
    ]);
    vi.mocked(listWorkoutSummaries).mockResolvedValue([
      {
        id: 'summary-legacy-event',
        workoutId: '990',
        rpe: 5,
        messages: [],
        savedAtEpochSeconds: null,
        createdAtEpochSeconds: 1,
        updatedAtEpochSeconds: 2,
      },
      {
        id: 'summary-activity',
        workoutId: 'activity-990',
        rpe: 7,
        messages: [],
        savedAtEpochSeconds: null,
        createdAtEpochSeconds: 3,
        updatedAtEpochSeconds: 4,
      },
    ]);

    const { result } = renderHook(() => useWorkoutList({ apiBaseUrl: '' }));

    await waitFor(() => {
      expect(result.current.state).toBe('ready');
    });

    expect(result.current.items).toHaveLength(1);
    expect(result.current.items[0]?.id).toBe('activity-990');
    expect(result.current.items[0]?.summary?.workoutId).toBe('activity-990');
    expect(listWorkoutSummaries).toHaveBeenCalledWith('', ['activity-990']);
  });

  it('uses an activity-only item when no related event exists', async () => {
    vi.mocked(listEvents).mockResolvedValue([]);
    vi.mocked(listActivities).mockResolvedValue([
      {
        ...activityFixture,
        id: 'activity-solo',
        name: 'Solo Ride',
        startDateLocal: '2026-03-29T07:00:00',
      },
    ]);
    vi.mocked(listWorkoutSummaries).mockResolvedValue([
      {
        id: 'summary-solo',
        workoutId: 'activity-solo',
        rpe: 4,
        messages: [],
        savedAtEpochSeconds: null,
        createdAtEpochSeconds: 1,
        updatedAtEpochSeconds: 2,
      },
    ]);

    const { result } = renderHook(() => useWorkoutList({ apiBaseUrl: '' }));

    await waitFor(() => {
      expect(result.current.state).toBe('ready');
    });

    expect(result.current.items[0]?.source).toBe('activity');
    expect(result.current.items[0]?.event).toBeNull();
    expect(result.current.items[0]?.activity?.id).toBe('activity-solo');
    expect(result.current.items[0]?.id).toBe('activity-solo');
    expect(result.current.items[0]?.summary?.workoutId).toBe('activity-solo');
  });

  it('marks missing intervals credentials as a dedicated state', async () => {
    vi.mocked(listActivities).mockResolvedValue([]);
    vi.mocked(listEvents).mockRejectedValue(new HttpError(422, 'bad request'));

    const { result } = renderHook(() => useWorkoutList({ apiBaseUrl: '' }));

    await waitFor(() => {
      expect(result.current.state).toBe('credentials-required');
    });
  });

  it('excludes planned-only event items from the visible list', async () => {
    vi.mocked(listActivities).mockResolvedValue([]);
    vi.mocked(listEvents).mockResolvedValue([
      {
        ...eventFixture,
        id: 500,
        name: 'Future Planned Workout',
        startDateLocal: '2026-04-01T09:00:00',
      },
    ]);
    vi.mocked(listWorkoutSummaries).mockResolvedValue([]);

    const { result } = renderHook(() => useWorkoutList({ apiBaseUrl: '' }));

    await waitFor(() => {
      expect(result.current.state).toBe('ready');
    });

    expect(result.current.items).toHaveLength(0);
  });

  it('includes past completed workouts and excludes future-dated ones', async () => {
    // Pin time to a specific date to make the test deterministic
    // April 15, 2026 is a Wednesday
    const pinnedDate = new Date('2026-04-15T12:00:00Z');
    vi.useFakeTimers({ toFake: ['Date'] });
    vi.setSystemTime(pinnedDate);

    try {
      // April 13 is Monday of the current week (past)
      // April 20 is Monday of next week (future, outside the visible week)
      vi.mocked(listActivities).mockResolvedValue([
        {
          ...activityFixture,
          id: 'activity-past',
          name: 'Past Ride',
          startDateLocal: '2026-04-13T09:00:00',
          startDate: '2026-04-13T08:00:00Z',
        },
        {
          ...activityFixture,
          id: 'activity-future',
          name: 'Future Ride',
          startDateLocal: '2026-04-20T09:00:00',
          startDate: '2026-04-20T08:00:00Z',
        },
      ]);
      vi.mocked(listEvents).mockResolvedValue([]);
      vi.mocked(listWorkoutSummaries).mockResolvedValue([]);

      const { result } = renderHook(() => useWorkoutList({ apiBaseUrl: '' }));

      // Advance timers to allow async operations to complete
      await act(async () => {
        await vi.advanceTimersByTimeAsync(100);
      });

      await waitFor(() => {
        expect(result.current.state).toBe('ready');
      }, { timeout: 1000 });

      // Only the past activity (April 13) should be visible
      // The future activity (April 20) is excluded because it's after today (April 15)
      expect(result.current.items).toHaveLength(1);
      expect(result.current.items[0]?.activity?.id).toBe('activity-past');
      expect(result.current.items[0]?.startDateLocal).toBe('2026-04-13T09:00:00');
    } finally {
      vi.useRealTimers();
    }
  });
});
