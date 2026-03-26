import { useCallback, useEffect, useMemo, useRef, useState } from 'react';

import { listActivities, listEvents } from '../../intervals/api/intervals';
import type { IntervalActivity, IntervalEvent } from '../../intervals/types';
import { HttpError } from '../../../lib/httpClient';
import {
  CALENDAR_BUFFER_WEEKS,
  CALENDAR_SHIFT_WEEKS,
  CALENDAR_WINDOW_WEEKS,
  CALENDAR_VISIBLE_WEEKS,
  CALENDAR_WEEK_ROW_GAP,
  CALENDAR_WEEK_ROW_HEIGHT,
} from '../constants';
import type {
  CalendarDataState,
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

type UseCalendarDataOptions = {
  apiBaseUrl: string;
};

type UseCalendarDataResult = {
  state: CalendarDataState;
  weeks: CalendarWeek[];
  topPreviewWeek: CalendarWeek;
  bottomPreviewWeek: CalendarWeek;
  isLoadingPast: boolean;
  isLoadingFuture: boolean;
  scrollAdjustment: CalendarScrollAdjustment;
  loadMorePast: () => Promise<void>;
  loadMoreFuture: () => Promise<void>;
};

type WeekStore = Map<string, CalendarWeek>;
type PaginationDirection = 'past' | 'future';

export function useCalendarData({ apiBaseUrl }: UseCalendarDataOptions): UseCalendarDataResult {
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
    if (paginationLockRef.current) {
      return false;
    }

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
    const [events, activities] = await Promise.all([
      listEvents(apiBaseUrl, range),
      listActivities(apiBaseUrl, range),
    ]);

    return { events, activities };
  }, [apiBaseUrl]);

  const hydrateWeeks = useCallback((startMonday: Date, count: number, events: IntervalEvent[], activities: IntervalActivity[], status: CalendarWeekStatus) => {
    const retainedWeekKeys = createRetainedWeekKeySet(windowStartRef.current);

    setStore((current) => {
      const next = new Map(current);
      for (let index = 0; index < count; index += 1) {
        const mondayDate = addWeeks(startMonday, index);
        const week = buildCalendarWeek(mondayDate, events, activities, status);
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
      for (let index = 0; index < count; index += 1) {
        const mondayDate = addWeeks(startMonday, index);
        const weekKey = toDateKey(mondayDate);
        const existing = next.get(weekKey);
        next.set(weekKey, existing ? { ...existing, status } : createPlaceholderWeek(mondayDate, status));
        inflightWeekKeysRef.current.add(weekKey);
      }
      return next;
    });
  }, []);

  const ensureWeeks = useCallback(async (startMonday: Date, count: number, placeholderStatus: CalendarWeekStatus = 'loading') => {
    const missingOffsets = Array.from({ length: count }, (_, index) => index).filter((index) => {
      const weekKey = toDateKey(addWeeks(startMonday, index));
      return !loadedWeekKeysRef.current.has(weekKey) && !inflightWeekKeysRef.current.has(weekKey);
    });

    if (missingOffsets.length === 0) {
      return;
    }

    reserveWeekOffsets(startMonday, missingOffsets, inflightWeekKeysRef.current);

    const ranges = groupContiguousOffsets(missingOffsets);

    for (const { startOffset, count: batchCount } of ranges) {
      const batchStart = addWeeks(startMonday, startOffset);

      markWeeks(batchStart, batchCount, placeholderStatus);

      try {
        const { events, activities } = await loadRange(batchStart, batchCount);
        hydrateWeeks(batchStart, batchCount, events, activities, 'loaded');
        setState('ready');
      } catch (error) {
        setStore((current) => {
          const retainedWeekKeys = createRetainedWeekKeySet(windowStartRef.current);
          const next = new Map(current);
          for (let index = 0; index < batchCount; index += 1) {
            const mondayDate = addWeeks(batchStart, index);
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

        if (error instanceof HttpError && error.status === 422) {
          setState('credentials-required');
        } else {
          setState((current) => (current === 'loading' ? 'error' : current));
        }
      }
    }
  }, [hydrateWeeks, loadRange, markWeeks]);

  const prefetchBuffer = useCallback(async (startMonday: Date) => {
    const bufferStart = addWeeks(startMonday, -CALENDAR_BUFFER_WEEKS);
    const total = CALENDAR_VISIBLE_WEEKS + (CALENDAR_BUFFER_WEEKS * 2);
    await ensureWeeks(bufferStart, total, 'idle');
  }, [ensureWeeks]);

  useEffect(() => {
    if (initializedRef.current) {
      return;
    }

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
    if (!beginPagination('past')) {
      return;
    }

    const currentWindowStart = windowStartRef.current;
    const nextWindowStart = addWeeks(currentWindowStart, -CALENDAR_SHIFT_WEEKS);
    const enteringStart = nextWindowStart;
    windowStartRef.current = nextWindowStart;
    setWindowStart(nextWindowStart);
    setScrollAdjustment((current) => ({
      topDelta: (CALENDAR_WEEK_ROW_HEIGHT + CALENDAR_WEEK_ROW_GAP) * CALENDAR_SHIFT_WEEKS,
      version: current.version + 1,
    }));

    try {
      // This asymmetry is intentional: loadMorePast shifts windowStart and scrollAdjustment
      // before awaiting ensureWeeks/prefetchBuffer so the user immediately sees the entering
      // placeholder week while scrolling upward. loadMoreFuture waits for data first to avoid
      // a visible downward jump when advancing the window.
      await ensureWeeks(enteringStart, CALENDAR_SHIFT_WEEKS);
      void prefetchBuffer(nextWindowStart);
    } finally {
      endPagination();
    }
  }, [beginPagination, endPagination, ensureWeeks, prefetchBuffer]);

  const loadMoreFuture = useCallback(async () => {
    if (!beginPagination('future')) {
      return;
    }

    const currentWindowStart = windowStartRef.current;
    const nextWindowStart = addWeeks(currentWindowStart, CALENDAR_SHIFT_WEEKS);

    try {
      await ensureWeeks(addWeeks(currentWindowStart, CALENDAR_VISIBLE_WEEKS), CALENDAR_SHIFT_WEEKS);
      windowStartRef.current = nextWindowStart;
      setWindowStart(nextWindowStart);
      setScrollAdjustment((current) => ({
        topDelta: -(CALENDAR_WEEK_ROW_HEIGHT + CALENDAR_WEEK_ROW_GAP) * CALENDAR_SHIFT_WEEKS,
        version: current.version + 1,
      }));
      void prefetchBuffer(nextWindowStart);
    } finally {
      endPagination();
    }
  }, [beginPagination, endPagination, ensureWeeks, prefetchBuffer]);

  const weeks = useMemo(() => {
    return Array.from({ length: CALENDAR_VISIBLE_WEEKS }, (_, index) => {
      const mondayDate = addWeeks(windowStart, index);
      const weekKey = toDateKey(mondayDate);
      return store.get(weekKey) ?? createPlaceholderWeek(mondayDate, 'loading');
    });
  }, [store, windowStart]);

  const topPreviewWeek = useMemo(() => {
    const mondayDate = addWeeks(windowStart, -1);
    return store.get(toDateKey(mondayDate)) ?? createPlaceholderWeek(mondayDate, 'loading');
  }, [store, windowStart]);

  const bottomPreviewWeek = useMemo(() => {
    const mondayDate = addWeeks(windowStart, CALENDAR_VISIBLE_WEEKS);
    return store.get(toDateKey(mondayDate)) ?? createPlaceholderWeek(mondayDate, 'loading');
  }, [store, windowStart]);

  return {
    state,
    weeks,
    topPreviewWeek,
    bottomPreviewWeek,
    isLoadingPast,
    isLoadingFuture,
    scrollAdjustment,
    loadMorePast,
    loadMoreFuture,
  };
}

function buildCalendarWeek(
  mondayDate: Date,
  events: IntervalEvent[],
  activities: IntervalActivity[],
  status: CalendarWeekStatus,
): CalendarWeek {
  const weekDates = generateWeekDates(mondayDate);
  const weekDateKeys = new Set(weekDates.map(toDateKey));
  const weekEvents = events.filter((event) => weekDateKeys.has(extractDateKey(event.startDateLocal)));
  const weekActivities = activities.filter((activity) => weekDateKeys.has(extractDateKey(activity.startDateLocal)));
  const days = weekDates.map((date) => buildCalendarDay(date, weekEvents, weekActivities));

  return {
    weekNumber: getWeekNumber(mondayDate),
    weekKey: toDateKey(mondayDate),
    mondayDate,
    days,
    summary: {
      totalTss: roundMetric(sumMetric(weekActivities, (activity) => activity.metrics.trainingStressScore)),
      targetTss: null,
      totalCalories: roundMetric(sumMetric(weekActivities, (activity) => activity.metrics.calories)),
      totalDurationSeconds: roundMetric(sumMetric(weekActivities, (activity) => activity.movingTimeSeconds)),
      targetDurationSeconds: null,
      totalDistanceMeters: sumMetric(weekActivities, (activity) => activity.distanceMeters),
    },
    status,
  };
}

function buildCalendarDay(date: Date, events: IntervalEvent[], activities: IntervalActivity[]): CalendarDay {
  const dateKey = toDateKey(date);

  return {
    date,
    dateKey,
    events: events.filter((event) => extractDateKey(event.startDateLocal) === dateKey),
    activities: activities.filter((activity) => extractDateKey(activity.startDateLocal) === dateKey),
  };
}

function createPlaceholderWeek(mondayDate: Date, status: CalendarWeekStatus): CalendarWeek {
  return {
    weekNumber: getWeekNumber(mondayDate),
    weekKey: toDateKey(mondayDate),
    mondayDate,
    days: generateWeekDates(mondayDate).map((date) => ({
      date,
      dateKey: toDateKey(date),
      events: [],
      activities: [],
    })),
    summary: {
      totalTss: 0,
      targetTss: null,
      totalCalories: 0,
      totalDurationSeconds: 0,
      targetDurationSeconds: null,
      totalDistanceMeters: 0,
    },
    status,
  };
}

function sumMetric<T>(items: T[], getValue: (item: T) => number | null): number {
  return items.reduce((total, item) => total + (getValue(item) ?? 0), 0);
}

function roundMetric(value: number): number {
  return Math.round(value);
}

function groupContiguousOffsets(offsets: number[]): Array<{ startOffset: number; count: number }> {
  if (offsets.length === 0) {
    return [];
  }

  const ranges: Array<{ startOffset: number; count: number }> = [];
  let rangeStart = offsets[0];
  let previous = offsets[0];
  let count = 1;

  for (let index = 1; index < offsets.length; index += 1) {
    const offset = offsets[index];
    if (offset === previous + 1) {
      count += 1;
    } else {
      ranges.push({ startOffset: rangeStart, count });
      rangeStart = offset;
      count = 1;
    }
    previous = offset;
  }

  ranges.push({ startOffset: rangeStart, count });
  return ranges;
}

function reserveWeekOffsets(startMonday: Date, offsets: number[], inflightWeekKeys: Set<string>) {
  for (const offset of offsets) {
    inflightWeekKeys.add(toDateKey(addWeeks(startMonday, offset)));
  }
}

function createRetainedWeekKeySet(windowStart: Date): Set<string> {
  const retainedStart = addWeeks(windowStart, -CALENDAR_BUFFER_WEEKS);

  return new Set(
    Array.from({ length: CALENDAR_WINDOW_WEEKS }, (_, index) => toDateKey(addWeeks(retainedStart, index))),
  );
}

function pruneWeekStore(store: WeekStore, retainedWeekKeys: Set<string>): WeekStore {
  const next = new Map<string, CalendarWeek>();

  for (const [weekKey, week] of store) {
    if (retainedWeekKeys.has(weekKey)) {
      next.set(weekKey, week);
    }
  }

  return next.size === store.size ? store : next;
}

function pruneWeekKeySet(weekKeys: Set<string>, retainedWeekKeys: Set<string>): Set<string> {
  const next = new Set<string>();

  for (const weekKey of weekKeys) {
    if (retainedWeekKeys.has(weekKey)) {
      next.add(weekKey);
    }
  }

  return next.size === weekKeys.size ? weekKeys : next;
}
