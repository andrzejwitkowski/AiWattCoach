import { act, renderHook, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import type { IntervalActivity, IntervalEvent } from '../../intervals/types';
import { AuthenticationError, HttpError } from '../../../lib/httpClient';
import type { CalendarLabel } from '../types';
import { CALENDAR_BUFFER_WEEKS, CALENDAR_VISIBLE_WEEKS } from '../constants';
import { addDays, getMondayOfWeek, parseDateKey, toDateKey } from '../utils/dateUtils';
import { useCalendarData } from './useCalendarData';

vi.mock('../../intervals/api/intervals', () => ({
  listCalendarEvents: vi.fn(),
  listActivities: vi.fn(),
  loadActivity: vi.fn(),
  loadEvent: vi.fn(),
}));

vi.mock('../api/calendar', () => ({
  listCalendarLabels: vi.fn(),
}));

import { listActivities, listCalendarEvents, loadActivity, loadEvent } from '../../intervals/api/intervals';
import { listCalendarLabels } from '../api/calendar';

const originalLocation = window.location;

function createDeferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((onResolve) => {
    resolve = onResolve;
  });

  return { promise, resolve };
}

function hasRangeCall(mock: ReturnType<typeof vi.fn>, oldest: string, newest: string): boolean {
  return countRangeCalls(mock, oldest, newest) > 0;
}

function countRangeCalls(mock: ReturnType<typeof vi.fn>, oldest: string, newest: string): number {
  return mock.mock.calls.filter(([, query]) => {
    return query !== null
      && typeof query === 'object'
      && 'oldest' in query
      && 'newest' in query
      && query.oldest === oldest
      && query.newest === newest;
  }).length;
}

afterEach(() => {
  vi.clearAllMocks();
  Object.defineProperty(window, 'location', {
    configurable: true,
    value: originalLocation,
  });
});

function mockNoDetailedEvents() {
  vi.mocked(loadEvent).mockResolvedValue(undefined as never);
}

function mockNoDetailedActivities() {
  vi.mocked(loadActivity).mockResolvedValue(undefined as never);
}

function mockNoCalendarLabels() {
  vi.mocked(listCalendarLabels).mockResolvedValue({ labelsByDate: {} });
}

describe('useCalendarData', () => {
  it('defaults unresolved weeks to idle placeholders', () => {
    const deferredEvents = createDeferred<IntervalEvent[]>();
    const deferredActivities = createDeferred<IntervalActivity[]>();
    const deferredLabels = createDeferred<{ labelsByDate: Record<string, Record<string, CalendarLabel>> }>();
    vi.mocked(listCalendarEvents).mockReturnValue(deferredEvents.promise);
    vi.mocked(listActivities).mockReturnValue(deferredActivities.promise);
    vi.mocked(listCalendarLabels).mockReturnValue(deferredLabels.promise);
    mockNoDetailedEvents();
    mockNoDetailedActivities();

    const { result, unmount } = renderHook(() => useCalendarData({ apiBaseUrl: '' }));

    expect(result.current.weeks.every((week) => week.status === 'idle')).toBe(true);
    expect(result.current.topPreviewWeek.status).toBe('idle');
    expect(result.current.bottomPreviewWeek.status).toBe('idle');

    unmount();
    deferredEvents.resolve([]);
    deferredActivities.resolve([]);
    deferredLabels.resolve({ labelsByDate: {} });
  });

  it('keeps a fixed five-week window after initial load', async () => {
    vi.mocked(listCalendarEvents).mockResolvedValue([] satisfies IntervalEvent[]);
    vi.mocked(listActivities).mockResolvedValue([] satisfies IntervalActivity[]);
    mockNoCalendarLabels();
    mockNoDetailedEvents();
    mockNoDetailedActivities();

    const { result } = renderHook(() => useCalendarData({ apiBaseUrl: '' }));

    await waitFor(() => {
      expect(result.current.state).toBe('ready');
    });

    expect(result.current.weeks).toHaveLength(CALENDAR_VISIBLE_WEEKS);
  });

  it('keeps five rendered weeks after scrolling forward', async () => {
    vi.mocked(listCalendarEvents).mockResolvedValue([] satisfies IntervalEvent[]);
    vi.mocked(listActivities).mockResolvedValue([] satisfies IntervalActivity[]);
    mockNoCalendarLabels();
    mockNoDetailedEvents();
    mockNoDetailedActivities();

    const { result } = renderHook(() => useCalendarData({ apiBaseUrl: '' }));

    await waitFor(() => {
      expect(result.current.state).toBe('ready');
    });

    const initialFirstWeek = result.current.weeks[0]?.weekKey;

    await act(async () => {
      await result.current.loadMoreFuture();
    });

    await waitFor(() => {
      expect(result.current.weeks).toHaveLength(CALENDAR_VISIBLE_WEEKS);
      expect(result.current.weeks[0]?.weekKey).not.toBe(initialFirstWeek);
    });
  });

  it('refetches weeks that were pruned from the buffer when scrolling back', async () => {
    vi.mocked(listCalendarEvents).mockResolvedValue([] satisfies IntervalEvent[]);
    vi.mocked(listActivities).mockResolvedValue([] satisfies IntervalActivity[]);
    mockNoCalendarLabels();
    mockNoDetailedEvents();
    mockNoDetailedActivities();

    const { result } = renderHook(() => useCalendarData({ apiBaseUrl: '' }));

    await waitFor(() => {
      expect(result.current.state).toBe('ready');
    });

    const initialFirstWeek = result.current.weeks[0]!.weekKey;
    const initialLastDay = toDateKey(addDays(parseDateKey(initialFirstWeek), 6));

    for (let index = 0; index < CALENDAR_BUFFER_WEEKS + 1; index += 1) {
      await act(async () => {
        await result.current.loadMoreFuture();
      });
    }

    vi.clearAllMocks();

    await act(async () => {
      await result.current.loadMorePast();
    });

    await waitFor(() => {
      expect(hasRangeCall(vi.mocked(listCalendarEvents), initialFirstWeek, initialLastDay)).toBe(true);
      expect(hasRangeCall(vi.mocked(listActivities), initialFirstWeek, initialLastDay)).toBe(true);
    });
  });

  it('coalesces concurrent forward loads into a single request', async () => {
    vi.mocked(listCalendarEvents).mockResolvedValue([] satisfies IntervalEvent[]);
    vi.mocked(listActivities).mockResolvedValue([] satisfies IntervalActivity[]);
    mockNoCalendarLabels();
    mockNoDetailedEvents();
    mockNoDetailedActivities();

    const { result } = renderHook(() => useCalendarData({ apiBaseUrl: '' }));

    await waitFor(() => {
      expect(result.current.state).toBe('ready');
    });

    const deferredEvents = createDeferred<IntervalEvent[]>();
    const deferredActivities = createDeferred<IntervalActivity[]>();
    const deferredLabels = createDeferred<{ labelsByDate: Record<string, Record<string, CalendarLabel>> }>();

    vi.clearAllMocks();
    vi.mocked(listCalendarEvents).mockReturnValueOnce(deferredEvents.promise);
    vi.mocked(listActivities).mockReturnValueOnce(deferredActivities.promise);
    vi.mocked(listCalendarLabels).mockReturnValueOnce(deferredLabels.promise);

    let firstLoad!: Promise<void>;
    let secondLoad!: Promise<void>;

    await act(async () => {
      firstLoad = result.current.loadMoreFuture();
      secondLoad = result.current.loadMoreFuture();
      await Promise.resolve();
    });

    expect(listCalendarEvents).toHaveBeenCalledTimes(1);
    expect(listActivities).toHaveBeenCalledTimes(1);
    expect(listCalendarLabels).toHaveBeenCalledTimes(1);

    deferredEvents.resolve([]);
    deferredActivities.resolve([]);
    deferredLabels.resolve({ labelsByDate: {} });

    await act(async () => {
      await Promise.all([firstLoad, secondLoad]);
    });
  });

  it('blocks an opposite-direction load while pagination is in flight', async () => {
    vi.mocked(listCalendarEvents).mockResolvedValue([] satisfies IntervalEvent[]);
    vi.mocked(listActivities).mockResolvedValue([] satisfies IntervalActivity[]);
    mockNoCalendarLabels();
    mockNoDetailedEvents();
    mockNoDetailedActivities();

    const { result } = renderHook(() => useCalendarData({ apiBaseUrl: '' }));

    await waitFor(() => {
      expect(result.current.state).toBe('ready');
    });

    for (let index = 0; index < CALENDAR_BUFFER_WEEKS; index += 1) {
      await act(async () => {
        await result.current.loadMoreFuture();
      });
    }

    const expectedFirstWeekAfterForward = result.current.weeks[1]!.weekKey;
    const deferredEvents = createDeferred<IntervalEvent[]>();
    const deferredActivities = createDeferred<IntervalActivity[]>();
    const deferredLabels = createDeferred<{ labelsByDate: Record<string, Record<string, CalendarLabel>> }>();

    vi.clearAllMocks();
    vi.mocked(listCalendarEvents).mockReturnValueOnce(deferredEvents.promise);
    vi.mocked(listActivities).mockReturnValueOnce(deferredActivities.promise);
    vi.mocked(listCalendarLabels).mockReturnValueOnce(deferredLabels.promise);

    let forwardLoad!: Promise<void>;
    let backwardLoad!: Promise<void>;

    await act(async () => {
      forwardLoad = result.current.loadMoreFuture();
      backwardLoad = result.current.loadMorePast();
      await Promise.resolve();
    });

    expect(listCalendarEvents).toHaveBeenCalledTimes(1);
    expect(listActivities).toHaveBeenCalledTimes(1);
    expect(listCalendarLabels).toHaveBeenCalledTimes(1);
    expect(result.current.weeks[0]!.weekKey).toBe(expectedFirstWeekAfterForward);

    deferredEvents.resolve([]);
    deferredActivities.resolve([]);
    deferredLabels.resolve({ labelsByDate: {} });

    await act(async () => {
      await Promise.all([forwardLoad, backwardLoad]);
    });
  });

  it('redirects to the landing page when calendar requests return unauthorized', async () => {
    vi.mocked(listCalendarEvents).mockRejectedValue(new AuthenticationError());
    vi.mocked(listActivities).mockResolvedValue([] satisfies IntervalActivity[]);
    mockNoCalendarLabels();
    mockNoDetailedEvents();
    mockNoDetailedActivities();

    Object.defineProperty(window, 'location', {
      configurable: true,
      value: { ...window.location, href: '/calendar' },
    });

    renderHook(() => useCalendarData({ apiBaseUrl: '' }));

    await waitFor(() => {
      expect(window.location.href).toBe('/');
    });
  });

  it('retries the same future range after a gateway failure on the next attempt', async () => {
    vi.mocked(listCalendarEvents).mockResolvedValue([] satisfies IntervalEvent[]);
    vi.mocked(listActivities).mockResolvedValue([] satisfies IntervalActivity[]);
    mockNoCalendarLabels();
    mockNoDetailedEvents();
    mockNoDetailedActivities();

    const { result } = renderHook(() => useCalendarData({ apiBaseUrl: '' }));

    await waitFor(() => {
      expect(result.current.state).toBe('ready');
    });

    const repeatedFailureWeek = toDateKey(
      addDays(result.current.bottomPreviewWeek.mondayDate, CALENDAR_BUFFER_WEEKS * 7)
    );
    const repeatedFailureWeekEnd = toDateKey(addDays(parseDateKey(repeatedFailureWeek), 6));

    vi.clearAllMocks();
    vi.mocked(listCalendarEvents).mockRejectedValue(new HttpError(502, 'bad gateway'));
    vi.mocked(listActivities).mockRejectedValue(new HttpError(502, 'bad gateway'));
    vi.mocked(listCalendarLabels).mockRejectedValue(new HttpError(502, 'bad gateway'));

    for (let attempt = 0; attempt < CALENDAR_BUFFER_WEEKS + 2; attempt += 1) {
      await act(async () => {
        await result.current.loadMoreFuture();
      });
    }

    expect(countRangeCalls(vi.mocked(listCalendarEvents), repeatedFailureWeek, repeatedFailureWeekEnd)).toBeGreaterThan(1);
    expect(countRangeCalls(vi.mocked(listActivities), repeatedFailureWeek, repeatedFailureWeekEnd)).toBeGreaterThan(1);
  });

  it('keeps workout events in the calendar window on list payloads only', async () => {
    const workoutDateKey = toDateKey(addDays(getMondayOfWeek(new Date()), 1));

    vi.mocked(listCalendarEvents).mockResolvedValue([
      {
        id: 55,
        startDateLocal: workoutDateKey,
        name: 'Threshold Builder',
        category: 'WORKOUT',
        description: null,
        restDay: false,
        restDayReason: null,
        indoor: true,
        color: null,
        eventDefinition: {
          rawWorkoutDoc: '- 3x10min',
          intervals: [{ definition: '- 3x10min', repeatCount: 3, durationSeconds: 600, targetPercentFtp: 92, zoneId: 4 }],
          segments: [],
          summary: {
            totalSegments: 0,
            totalDurationSeconds: 1800,
            estimatedNormalizedPowerWatts: null,
            estimatedAveragePowerWatts: null,
            estimatedIntensityFactor: 0.92,
            estimatedTrainingStressScore: 64,
          },
        },
        actualWorkout: null,
      },
    ] satisfies IntervalEvent[]);
    vi.mocked(listActivities).mockResolvedValue([] satisfies IntervalActivity[]);
    mockNoCalendarLabels();
    mockNoDetailedActivities();
    mockNoDetailedEvents();

    const { result } = renderHook(() => useCalendarData({ apiBaseUrl: '' }));

    await waitFor(() => {
      expect(result.current.state).toBe('ready');
    });

    const workoutDay = result.current.weeks.flatMap((week) => week.days).find((day) => day.dateKey === workoutDateKey);

    expect(loadEvent).not.toHaveBeenCalled();
    expect(workoutDay?.events[0]?.eventDefinition.segments).toHaveLength(0);
  });

  it('hydrates predicted workouts with positive safe synthetic ids', async () => {
    const workoutDateKey = toDateKey(addDays(getMondayOfWeek(new Date()), 1));

    vi.mocked(listCalendarEvents).mockResolvedValue([
      {
        id: 5906112577594034,
        calendarEntryId: 'predicted:training-plan:user-1:w1:1775719860:2026-04-11',
        startDateLocal: workoutDateKey,
        name: 'Active Recovery',
        category: 'WORKOUT',
        description: null,
        restDay: false,
        restDayReason: null,
        indoor: false,
        color: null,
        eventDefinition: {
          rawWorkoutDoc: 'Active Recovery\n- 45m 50%',
          intervals: [
            { definition: 'Active Recovery', repeatCount: 1, durationSeconds: null, targetPercentFtp: null, zoneId: null },
            { definition: '- 45m 50%', repeatCount: 1, durationSeconds: 2700, targetPercentFtp: 50, zoneId: 1 },
          ],
          segments: [
            { order: 0, label: '45m 50%', durationSeconds: 2700, startOffsetSeconds: 0, endOffsetSeconds: 2700, targetPercentFtp: 50, zoneId: 1 },
          ],
          summary: {
            totalSegments: 1,
            totalDurationSeconds: 2700,
            estimatedNormalizedPowerWatts: null,
            estimatedAveragePowerWatts: null,
            estimatedIntensityFactor: 0.5,
            estimatedTrainingStressScore: 19,
          },
        },
        actualWorkout: null,
        plannedSource: 'predicted',
        syncStatus: 'unsynced',
        projectedWorkout: {
          projectedWorkoutId: 'training-plan:user-1:w1:1775719860:2026-04-11',
          operationKey: 'training-plan:user-1:w1:1775719860',
          date: workoutDateKey,
          sourceWorkoutId: 'w1',
          restDay: false,
        },
      },
    ] satisfies IntervalEvent[]);
    vi.mocked(listActivities).mockResolvedValue([] satisfies IntervalActivity[]);
    mockNoCalendarLabels();
    mockNoDetailedEvents();
    mockNoDetailedActivities();

    const { result } = renderHook(() => useCalendarData({ apiBaseUrl: '' }));

    await waitFor(() => {
      expect(result.current.state).toBe('ready');
    });

    const workoutDay = result.current.weeks.flatMap((week) => week.days).find((day) => day.dateKey === workoutDateKey);

    expect(workoutDay?.events).toHaveLength(1);
    expect(workoutDay?.events[0]?.id).toBe(5906112577594034);
    expect(workoutDay?.events[0]?.name).toBe('Active Recovery');
  });

  it('keeps completed workout activities in the calendar window on list payloads only', async () => {
    const workoutDateKey = toDateKey(addDays(getMondayOfWeek(new Date()), 2));

    vi.mocked(listCalendarEvents).mockResolvedValue([] satisfies IntervalEvent[]);
    vi.mocked(listActivities).mockResolvedValue([
      {
        id: 'a55',
        startDateLocal: `${workoutDateKey}T08:00:00`,
        startDate: `${workoutDateKey}T07:00:00Z`,
        name: 'Wild Snow',
        description: null,
        activityType: 'Ride',
        source: null,
        externalId: null,
        deviceName: null,
        distanceMeters: 42000,
        movingTimeSeconds: 5820,
        elapsedTimeSeconds: 5820,
        totalElevationGainMeters: null,
        averageSpeedMps: null,
        averageHeartRateBpm: null,
        averageCadenceRpm: null,
        trainer: false,
        commute: false,
        race: false,
        hasHeartRate: true,
        streamTypes: [],
        tags: [],
        metrics: {
          trainingStressScore: 101,
          normalizedPowerWatts: 249,
          intensityFactor: null,
          efficiencyFactor: null,
          variabilityIndex: null,
          averagePowerWatts: null,
          ftpWatts: 280,
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
        detailsUnavailableReason: null,
      },
    ] satisfies IntervalActivity[]);
    mockNoCalendarLabels();
    mockNoDetailedEvents();
    mockNoDetailedActivities();

    const { result } = renderHook(() => useCalendarData({ apiBaseUrl: '' }));

    await waitFor(() => {
      expect(result.current.state).toBe('ready');
    });

    const workoutDay = result.current.weeks.flatMap((week) => week.days).find((day) => day.dateKey === workoutDateKey);

    expect(loadActivity).not.toHaveBeenCalled();
    expect(workoutDay?.activities[0]?.details.intervals).toHaveLength(0);
  });

  it('does not hydrate event or activity details during range loading', async () => {
    vi.mocked(listCalendarEvents).mockResolvedValue([] satisfies IntervalEvent[]);
    vi.mocked(listActivities).mockResolvedValue([] satisfies IntervalActivity[]);
    mockNoCalendarLabels();
    mockNoDetailedEvents();
    mockNoDetailedActivities();

    const { result } = renderHook(() => useCalendarData({ apiBaseUrl: '' }));

    await waitFor(() => {
      expect(result.current.state).toBe('ready');
    });

    expect(loadEvent).not.toHaveBeenCalled();
    expect(loadActivity).not.toHaveBeenCalled();
  });

  it('hydrates race labels into matching calendar days', async () => {
    const raceDateKey = toDateKey(addDays(getMondayOfWeek(new Date()), 3));

    vi.mocked(listCalendarEvents).mockResolvedValue([] satisfies IntervalEvent[]);
    vi.mocked(listActivities).mockResolvedValue([] satisfies IntervalActivity[]);
    vi.mocked(listCalendarLabels).mockResolvedValue({
      labelsByDate: {
        [raceDateKey]: {
          'race:race-1': {
            kind: 'race',
            title: 'Race Gravel Attack',
            subtitle: '120 km • Kat. A',
            payload: {
              raceId: 'race-1',
              date: raceDateKey,
              name: 'Gravel Attack',
              distanceMeters: 120000,
              discipline: 'gravel',
              priority: 'A',
              syncStatus: 'synced',
              linkedIntervalsEventId: 41,
            },
          },
        },
      },
    });
    mockNoDetailedEvents();
    mockNoDetailedActivities();

    const { result } = renderHook(() => useCalendarData({ apiBaseUrl: '' }));

    await waitFor(() => {
      expect(result.current.state).toBe('ready');
    });

    const raceDay = result.current.weeks.flatMap((week) => week.days).find((day) => day.dateKey === raceDateKey);

    expect(raceDay?.labels).toHaveLength(1);
    expect(raceDay?.labels[0]?.kind).toBe('race');
    if (raceDay?.labels[0]?.kind === 'race') {
      expect(raceDay.labels[0].payload.priority).toBe('A');
    }
  });

});
