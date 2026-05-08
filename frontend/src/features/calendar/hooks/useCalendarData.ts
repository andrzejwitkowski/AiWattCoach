import { useCallback, useEffect, useMemo, useRef, useState } from 'react';

import { listCalendarLabels } from '../api/calendar';
import { listCalendarEvents } from '../../intervals/api/intervals';
import { useCompletedWorkouts } from '../../intervals/context';
import type { IntervalActivity, IntervalEvent } from '../../intervals/types';
import { AuthenticationError, HttpError } from '../../../lib/httpClient';
import {
  CALENDAR_BUFFER_WEEKS,
  CALENDAR_SHIFT_WEEKS,
  CALENDAR_WEEK_BLOCK_HEIGHT,
  CALENDAR_WINDOW_WEEKS,
  CALENDAR_VISIBLE_WEEKS,
} from '../constants';
import type {
  CalendarDataState,
  CalendarLabel,
  CalendarDay,
  CalendarScrollAdjustment,
  CalendarWeek,
  CalendarWeekStatus,
} from '../types';
import {
  addWeeks,
  extractDateKey,
  formatDateRange,
  generateWeekDates,
  getMondayOfWeek,
  getWeekNumber,
  toDateKey,
} from '../utils/dateUtils';

const CACHE_TTL_MS = 5 * 60 * 1000;

type CachedRange<T> = { data: T; loadedAt: number };
type LabelsResponse = { labelsByDate: Record<string, Record<string, CalendarLabel>> };

const eventsCacheRef: Map<string, CachedRange<IntervalEvent[]>> = new Map();
const labelsCacheRef: Map<string, CachedRange<LabelsResponse>> = new Map();

const isStale = (loadedAt: number) => Date.now() - loadedAt > CACHE_TTL_MS;

async function fetchEventsWithCache(apiBaseUrl: string, range: { oldest: string; newest: string }): Promise<IntervalEvent[]> {
  const key = `${apiBaseUrl}|${range.oldest}|${range.newest}`;
  const cached = eventsCacheRef.get(key);
  if (cached && !isStale(cached.loadedAt)) return cached.data;
  const data = await listCalendarEvents(apiBaseUrl, range);
  eventsCacheRef.set(key, { data, loadedAt: Date.now() });
  return data;
}

async function fetchLabelsWithCache(apiBaseUrl: string, range: { oldest: string; newest: string }): Promise<LabelsResponse> {
  const key = `${apiBaseUrl}|${range.oldest}|${range.newest}`;
  const cached = labelsCacheRef.get(key);
  if (cached && !isStale(cached.loadedAt)) return cached.data;
  const data = await listCalendarLabels(apiBaseUrl, range);
  labelsCacheRef.set(key, { data, loadedAt: Date.now() });
  return data;
}

type UseCalendarDataOptions = { apiBaseUrl: string };

type UseCalendarDataResult = {
  state: CalendarDataState;
  weeks: CalendarWeek[];
  renderedWeeks: CalendarWeek[];
  topPreviewWeek: CalendarWeek;
  bottomPreviewWeek: CalendarWeek;
  isLoadingPast: boolean;
  isLoadingFuture: boolean;
  scrollAdjustment: CalendarScrollAdjustment;
  loadMorePast: () => Promise<void>;
  loadMoreFuture: () => Promise<void>;
  replaceEvent: (nextEvent: IntervalEvent) => void;
};

type WeekStore = Map<string, CalendarWeek>;
type PaginationDirection = 'past' | 'future';

export function useCalendarData({ apiBaseUrl }: UseCalendarDataOptions): UseCalendarDataResult {
  const { getActivitiesForRange } = useCompletedWorkouts();
  const [state, setState] = useState<CalendarDataState>('loading');
  const [store, setStore] = useState<WeekStore>(new Map());
  const [windowStart, setWindowStart] = useState<Date>(() => getMondayOfWeek(new Date()));
  const [isLoadingPast, setIsLoadingPast] = useState(false);
  const [isLoadingFuture, setIsLoadingFuture] = useState(false);
  const [scrollAdjustment, setScrollAdjustment] = useState<CalendarScrollAdjustment>({ topDelta: 0, version: 0 });
  const loadedWeekKeysRef = useRef<Set<string>>(new Set());
  const inflightWeekKeysRef = useRef<Set<string>>(new Set());
  const paginationLockRef = useRef(false);
  const initializedRef = useRef(false);
  const windowStartRef = useRef(windowStart);

  const beginPagination = useCallback((direction: PaginationDirection): boolean => {
    if (paginationLockRef.current) return false;
    paginationLockRef.current = true;
    setIsLoadingPast(direction === 'past');
    setIsLoadingFuture(direction === 'future');
    return true;
  }, []);

  const endPagination = useCallback(() => {
    paginationLockRef.current = false;
    setIsLoadingPast(false);
    setIsLoadingFuture(false);
  }, []);

  const pruneStoredWeeks = useCallback((anchorStart: Date) => {
    const retainedWeekKeys = createRetainedWeekKeySet(anchorStart);
    setStore((current) => pruneWeekStore(current, retainedWeekKeys));
    loadedWeekKeysRef.current = pruneWeekKeySet(loadedWeekKeysRef.current, retainedWeekKeys);
    inflightWeekKeysRef.current = pruneWeekKeySet(inflightWeekKeysRef.current, retainedWeekKeys);
  }, []);

  const loadRange = useCallback(async (startMonday: Date, count: number) => {
    const range = formatDateRange(startMonday, count);
    const [events, activities, labels] = await Promise.all([
      fetchEventsWithCache(apiBaseUrl, range),
      getActivitiesForRange(range.oldest, range.newest),
      fetchLabelsWithCache(apiBaseUrl, range),
    ]);
    return { events, activities, labels };
  }, [apiBaseUrl, getActivitiesForRange]);

  const hydrateWeeks = useCallback((
    startMonday: Date,
    count: number,
    events: IntervalEvent[],
    activities: IntervalActivity[],
    labels: Record<string, Record<string, CalendarLabel>>,
    status: CalendarWeekStatus,
  ) => {
    const retainedWeekKeys = createRetainedWeekKeySet(windowStartRef.current);
    const eventsByDateKey = groupItemsByDateKey(events, (e) => extractDateKey(e.startDateLocal));
    const activitiesByDateKey = groupItemsByDateKey(activities, (a) => extractDateKey(a.startDateLocal));
    const labelsByDateKey = groupLabelsByDateKey(labels);

    setStore((current) => {
      const next = new Map(current);
      for (let i = 0; i < count; i += 1) {
        const mondayDate = addWeeks(startMonday, i);
        const week = buildCalendarWeek(mondayDate, eventsByDateKey, activitiesByDateKey, labelsByDateKey, status);
        if (retainedWeekKeys.has(week.weekKey)) {
          next.set(week.weekKey, week);
          loadedWeekKeysRef.current.add(week.weekKey);
        } else {
          next.delete(week.weekKey);
          loadedWeekKeysRef.current.delete(week.weekKey);
        }
        inflightWeekKeysRef.current.delete(week.weekKey);
      }
      return next;
    });
  }, []);

  const markWeeks = useCallback((startMonday: Date, count: number, status: CalendarWeekStatus) => {
    setStore((current) => {
      const next = new Map(current);
      for (let i = 0; i < count; i += 1) {
        const mondayDate = addWeeks(startMonday, i);
        const weekKey = toDateKey(mondayDate);
        const existing = next.get(weekKey);
        next.set(weekKey, existing ? { ...existing, status } : createPlaceholderWeek(mondayDate, status));
        inflightWeekKeysRef.current.add(weekKey);
      }
      return next;
    });
  }, []);

  const ensureWeeks = useCallback(async (startMonday: Date, count: number, placeholderStatus: CalendarWeekStatus = 'loading') => {
    const missingOffsets = Array.from({ length: count }, (_, i) => i).filter((i) => {
      const weekKey = toDateKey(addWeeks(startMonday, i));
      return !loadedWeekKeysRef.current.has(weekKey) && !inflightWeekKeysRef.current.has(weekKey);
    });

    if (missingOffsets.length === 0) return;

    reserveWeekOffsets(startMonday, missingOffsets, inflightWeekKeysRef.current);

    for (const { startOffset, count: batchCount } of groupContiguousOffsets(missingOffsets)) {
      const batchStart = addWeeks(startMonday, startOffset);
      markWeeks(batchStart, batchCount, placeholderStatus);

      try {
        const { events, activities, labels } = await loadRange(batchStart, batchCount);
        hydrateWeeks(batchStart, batchCount, events, activities, labels.labelsByDate, 'loaded');
        setState('ready');
      } catch (error) {
        setStore((current) => {
          const retainedWeekKeys = createRetainedWeekKeySet(windowStartRef.current);
          const next = new Map(current);
          for (let i = 0; i < batchCount; i += 1) {
            const mondayDate = addWeeks(batchStart, i);
            const weekKey = toDateKey(mondayDate);
            if (retainedWeekKeys.has(weekKey)) {
              next.set(weekKey, createPlaceholderWeek(mondayDate, 'error'));
            } else {
              next.delete(weekKey);
            }
            inflightWeekKeysRef.current.delete(weekKey);
            loadedWeekKeysRef.current.delete(weekKey);
          }
          return next;
        });

        if (error instanceof AuthenticationError) {
          window.location.href = '/';
        } else if (error instanceof HttpError && error.status === 422) {
          setState('credentials-required');
        } else {
          setState((current) => (current === 'loading' ? 'error' : current));
        }
      }
    }
  }, [hydrateWeeks, loadRange, markWeeks]);

  const prefetchBuffer = useCallback(async (startMonday: Date) => {
    const bufferStart = addWeeks(startMonday, -CALENDAR_BUFFER_WEEKS);
    await ensureWeeks(bufferStart, CALENDAR_VISIBLE_WEEKS + CALENDAR_BUFFER_WEEKS * 2, 'idle');
  }, [ensureWeeks]);

  useEffect(() => {
    if (initializedRef.current) return;
    initializedRef.current = true;
    const initialStart = getMondayOfWeek(new Date());
    setWindowStart(initialStart);
    void prefetchBuffer(initialStart);
  }, [ensureWeeks, prefetchBuffer]);

  useEffect(() => {
    windowStartRef.current = windowStart;
    pruneStoredWeeks(windowStart);
  }, [pruneStoredWeeks, windowStart]);

  const loadMorePast = useCallback(async () => {
    if (!beginPagination('past')) return;
    const currentWindowStart = windowStartRef.current;
    const nextWindowStart = addWeeks(currentWindowStart, -CALENDAR_SHIFT_WEEKS);
    const enteringWeekKey = toDateKey(nextWindowStart);

    try {
      await ensureWeeks(nextWindowStart, CALENDAR_SHIFT_WEEKS);
      if (!loadedWeekKeysRef.current.has(enteringWeekKey)) return;
      windowStartRef.current = nextWindowStart;
      setWindowStart(nextWindowStart);
      setScrollAdjustment((c) => ({ topDelta: CALENDAR_WEEK_BLOCK_HEIGHT * CALENDAR_SHIFT_WEEKS, version: c.version + 1 }));
      void prefetchBuffer(nextWindowStart);
    } finally {
      endPagination();
    }
  }, [beginPagination, endPagination, ensureWeeks, prefetchBuffer]);

  const loadMoreFuture = useCallback(async () => {
    if (!beginPagination('future')) return;
    const currentWindowStart = windowStartRef.current;
    const nextWindowStart = addWeeks(currentWindowStart, CALENDAR_SHIFT_WEEKS);
    const enteringStart = addWeeks(currentWindowStart, CALENDAR_VISIBLE_WEEKS);
    const enteringWeekKey = toDateKey(enteringStart);

    try {
      await ensureWeeks(enteringStart, CALENDAR_SHIFT_WEEKS);
      if (!loadedWeekKeysRef.current.has(enteringWeekKey)) return;
      windowStartRef.current = nextWindowStart;
      setWindowStart(nextWindowStart);
      setScrollAdjustment((c) => ({ topDelta: -(CALENDAR_WEEK_BLOCK_HEIGHT * CALENDAR_SHIFT_WEEKS), version: c.version + 1 }));
      void prefetchBuffer(nextWindowStart);
    } finally {
      endPagination();
    }
  }, [beginPagination, endPagination, ensureWeeks, prefetchBuffer]);

  const replaceEvent = useCallback((nextEvent: IntervalEvent) => {
    const nextDateKey = extractDateKey(nextEvent.startDateLocal);

    setStore((current) => {
      let changed = false;
      const next = new Map(current);

      for (const [weekKey, week] of current) {
        let weekChanged = false;
        const days = week.days.map((day) => {
          const matchingEventIndex = day.events.findIndex((event) => isSameCalendarEvent(event, nextEvent));
          if (matchingEventIndex === -1) {
            return day;
          }

          weekChanged = true;
          changed = true;

          const remainingEvents = day.events.filter((_, index) => index !== matchingEventIndex);
          const nextEvents = day.dateKey === nextDateKey
            ? insertEventSorted([...remainingEvents, nextEvent])
            : remainingEvents;

          return {
            ...day,
            events: nextEvents,
          };
        });

        if (weekChanged) {
          next.set(weekKey, {
            ...week,
            days,
          });
        }
      }

      return changed ? next : current;
    });

    invalidateCalendarEventCache(nextDateKey);
  }, []);

  const weeks = useMemo(() =>
    Array.from({ length: CALENDAR_VISIBLE_WEEKS }, (_, i) => {
      const mondayDate = addWeeks(windowStart, i);
      return store.get(toDateKey(mondayDate)) ?? createPlaceholderWeek(mondayDate, 'idle');
    }), [store, windowStart]);

  const renderedWeeks = useMemo(() => {
    const renderedStart = addWeeks(windowStart, -CALENDAR_BUFFER_WEEKS);
    return Array.from({ length: CALENDAR_WINDOW_WEEKS }, (_, i) => {
      const mondayDate = addWeeks(renderedStart, i);
      return store.get(toDateKey(mondayDate)) ?? createPlaceholderWeek(mondayDate, 'idle');
    });
  }, [store, windowStart]);

  const topPreviewWeek = useMemo(() =>
    store.get(toDateKey(addWeeks(windowStart, -1))) ?? createPlaceholderWeek(addWeeks(windowStart, -1), 'idle'),
    [store, windowStart]);

  const bottomPreviewWeek = useMemo(() =>
    store.get(toDateKey(addWeeks(windowStart, CALENDAR_VISIBLE_WEEKS))) ?? createPlaceholderWeek(addWeeks(windowStart, CALENDAR_VISIBLE_WEEKS), 'idle'),
    [store, windowStart]);

  return {
    state,
    weeks,
    renderedWeeks,
    topPreviewWeek,
    bottomPreviewWeek,
    isLoadingPast,
    isLoadingFuture,
    scrollAdjustment,
    loadMorePast,
    loadMoreFuture,
    replaceEvent,
  };
}

function buildCalendarWeek(
  mondayDate: Date,
  eventsByDateKey: Map<string, IntervalEvent[]>,
  activitiesByDateKey: Map<string, IntervalActivity[]>,
  labelsByDateKey: Map<string, CalendarLabel[]>,
  status: CalendarWeekStatus,
): CalendarWeek {
  const weekDates = generateWeekDates(mondayDate);
  const weekDateKeys = weekDates.map(toDateKey);
  const weekActivities = weekDateKeys.flatMap((k) => activitiesByDateKey.get(k) ?? []);

  return {
    weekNumber: getWeekNumber(mondayDate),
    weekKey: toDateKey(mondayDate),
    mondayDate,
    days: weekDates.map((d) => buildCalendarDay(d, eventsByDateKey, activitiesByDateKey, labelsByDateKey)),
    summary: {
      totalTss: roundMetric(sumMetric(weekActivities, (a) => a.metrics.trainingStressScore)),
      targetTss: null,
      totalCalories: roundMetric(sumMetric(weekActivities, (a) => a.metrics.calories)),
      totalDurationSeconds: roundMetric(sumMetric(weekActivities, (a) => a.movingTimeSeconds)),
      targetDurationSeconds: null,
      totalDistanceMeters: sumMetric(weekActivities, (a) => a.distanceMeters),
    },
    status,
  };
}

function buildCalendarDay(
  date: Date,
  eventsByDateKey: Map<string, IntervalEvent[]>,
  activitiesByDateKey: Map<string, IntervalActivity[]>,
  labelsByDateKey: Map<string, CalendarLabel[]>,
): CalendarDay {
  const dateKey = toDateKey(date);
  return {
    date,
    dateKey,
    events: eventsByDateKey.get(dateKey) ?? [],
    activities: activitiesByDateKey.get(dateKey) ?? [],
    labels: labelsByDateKey.get(dateKey) ?? [],
  };
}

const groupLabelsByDateKey = (labelsByDate: Record<string, Record<string, CalendarLabel>>): Map<string, CalendarLabel[]> => {
  const grouped = new Map<string, CalendarLabel[]>();
  for (const [dateKey, labels] of Object.entries(labelsByDate))
    grouped.set(dateKey, Object.values(labels));
  return grouped;
};

function groupItemsByDateKey<T>(items: T[], getDateKey: (item: T) => string): Map<string, T[]> {
  const grouped = new Map<string, T[]>();
  for (const item of items) {
    const dateKey = getDateKey(item);
    const existing = grouped.get(dateKey);
    if (existing) existing.push(item);
    else grouped.set(dateKey, [item]);
  }
  return grouped;
}

function createPlaceholderWeek(mondayDate: Date, status: CalendarWeekStatus): CalendarWeek {
  return {
    weekNumber: getWeekNumber(mondayDate),
    weekKey: toDateKey(mondayDate),
    mondayDate,
    days: generateWeekDates(mondayDate).map((d) => ({
      date: d,
      dateKey: toDateKey(d),
      events: [],
      activities: [],
      labels: [],
    })),
    summary: { totalTss: 0, targetTss: null, totalCalories: 0, totalDurationSeconds: 0, targetDurationSeconds: null, totalDistanceMeters: 0 },
    status,
  };
}

const sumMetric = <T,>(items: T[], getValue: (item: T) => number | null): number =>
  items.reduce((total, item) => total + (getValue(item) ?? 0), 0);

const roundMetric = (value: number): number => Math.round(value);

function groupContiguousOffsets(offsets: number[]): Array<{ startOffset: number; count: number }> {
  if (offsets.length === 0) return [];

  const ranges: Array<{ startOffset: number; count: number }> = [];
  let rangeStart = offsets[0];
  let previous = offsets[0];
  let count = 1;

  for (let i = 1; i < offsets.length; i += 1) {
    if (offsets[i] === previous + 1) count += 1;
    else {
      ranges.push({ startOffset: rangeStart, count });
      rangeStart = offsets[i];
      count = 1;
    }
    previous = offsets[i];
  }

  ranges.push({ startOffset: rangeStart, count });
  return ranges;
}

const reserveWeekOffsets = (startMonday: Date, offsets: number[], inflightWeekKeys: Set<string>) => {
  for (const offset of offsets)
    inflightWeekKeys.add(toDateKey(addWeeks(startMonday, offset)));
};

const createRetainedWeekKeySet = (windowStart: Date): Set<string> =>
  new Set(Array.from({ length: CALENDAR_WINDOW_WEEKS }, (_, i) => toDateKey(addWeeks(windowStart, -CALENDAR_BUFFER_WEEKS + i))));

function pruneWeekStore(store: WeekStore, retainedWeekKeys: Set<string>): WeekStore {
  const next = new Map<string, CalendarWeek>();
  for (const [weekKey, week] of store)
    if (retainedWeekKeys.has(weekKey)) next.set(weekKey, week);
  return next.size === store.size ? store : next;
}

function pruneWeekKeySet(weekKeys: Set<string>, retainedWeekKeys: Set<string>): Set<string> {
  const next = new Set<string>();
  for (const weekKey of weekKeys)
    if (retainedWeekKeys.has(weekKey)) next.add(weekKey);
  return next.size === weekKeys.size ? weekKeys : next;
}

export function invalidateCalendarCache() {
  eventsCacheRef.clear();
  labelsCacheRef.clear();
}

function invalidateCalendarEventCache(dateKey: string) {
  for (const cacheKey of eventsCacheRef.keys()) {
    const [, oldest, newest] = cacheKey.split('|');
    if (!oldest || !newest) {
      continue;
    }

    if (dateKey >= oldest && dateKey <= newest) {
      eventsCacheRef.delete(cacheKey);
    }
  }
}

function isSameCalendarEvent(currentEvent: IntervalEvent, nextEvent: IntervalEvent): boolean {
  const currentProjectedWorkoutId = currentEvent.projectedWorkout?.projectedWorkoutId;
  const nextProjectedWorkoutId = nextEvent.projectedWorkout?.projectedWorkoutId;
  if (currentProjectedWorkoutId && nextProjectedWorkoutId) {
    return currentProjectedWorkoutId === nextProjectedWorkoutId;
  }

  return currentEvent.calendarEntryId === nextEvent.calendarEntryId;
}

function insertEventSorted(events: IntervalEvent[]): IntervalEvent[] {
  return [...events].sort((left, right) => {
    if (left.startDateLocal !== right.startDateLocal) {
      return left.startDateLocal.localeCompare(right.startDateLocal);
    }

    return left.id - right.id;
  });
}

export const __resetCachesForTesting = invalidateCalendarCache;
