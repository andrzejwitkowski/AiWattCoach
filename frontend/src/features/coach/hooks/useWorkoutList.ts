import { useCallback, useEffect, useMemo, useRef, useState } from 'react';

import { listEvents } from '../../intervals/api/intervals';
import { useCompletedWorkouts } from '../../intervals/context';
import { AuthenticationError, HttpError } from '../../../lib/httpClient';
import { addDays, addWeeks, extractDateKey, formatDateRange, getMondayOfWeek, toDateKey } from '../../calendar/utils/dateUtils';
import { listWorkoutSummaries } from '../api/workoutSummary';
import type { CoachWorkoutListItem, WorkoutSummary } from '../types';
import type { IntervalActivity, IntervalEvent } from '../../intervals/types';

export type WorkoutListState = 'loading' | 'ready' | 'error' | 'credentials-required';

const WORKOUT_PAGE_SIZE = 7;
const WORKOUT_LOOKBACK_WEEKS = 12;
const MAX_SUMMARY_BATCH_SIZE = 31;

type UseWorkoutListOptions = {
  apiBaseUrl: string;
};

type UseWorkoutListResult = {
  items: CoachWorkoutListItem[];
  state: WorkoutListState;
  error: string | null;
  weekLabel: string;
  canGoToNewerWeek: boolean;
  goToOlderWeek: () => void;
  goToNewerWeek: () => void;
  refresh: () => Promise<void>;
  replaceSummary: (summary: WorkoutSummary) => void;
};

function formatRangeLabel(startDate: Date, endDate: Date): string {
  const formatter = new Intl.DateTimeFormat(undefined, {
    month: 'short',
    day: 'numeric',
  });
  return `${formatter.format(startDate)} - ${formatter.format(endDate)}`;
}

function normalizeName(value: string | null | undefined): string {
  return value?.trim().toLowerCase().replace(/\s+/g, ' ') ?? '';
}

function namesLookRelated(event: IntervalEvent, activity: IntervalActivity): boolean {
  const eventName = normalizeName(event.name);
  const activityName = normalizeName(activity.name) || normalizeName(activity.activityType);

  if (!eventName || !activityName) {
    return false;
  }

  return eventName === activityName || eventName.includes(activityName) || activityName.includes(eventName);
}

function chooseMatchedActivity(
  event: IntervalEvent,
  candidates: IntervalActivity[],
  dayEventCount: number,
  dayActivityCount: number,
): IntervalActivity | null {
  const namedCandidates = candidates.filter((activity) => namesLookRelated(event, activity));

  if (namedCandidates.length === 1) {
    return namedCandidates[0] ?? null;
  }

  if (candidates.length === 1 && dayEventCount === 1 && dayActivityCount === 1) {
    return candidates[0] ?? null;
  }

  return null;
}

function inferEventIdHint(activity: IntervalActivity): string | null {
  const values = [activity.externalId, activity.description, activity.name];

  for (const value of values) {
    const match = value?.match(/paired_event_id=(\d+)/i);
    if (match?.[1]) {
      return match[1];
    }
  }

  return null;
}

function buildWorkoutItems(
  events: IntervalEvent[],
  activities: IntervalActivity[],
): CoachWorkoutListItem[] {
  const eventsSorted = [...events].sort((left, right) => right.startDateLocal.localeCompare(left.startDateLocal));
  const activitiesSorted = [...activities].sort((left, right) => right.startDateLocal.localeCompare(left.startDateLocal));
  const activitiesByDate = new Map<string, IntervalActivity[]>();
  const eventsByDate = new Map<string, IntervalEvent[]>();
  const eventsByActualWorkoutActivityId = new Map<string, IntervalEvent[]>();

  for (const activity of activitiesSorted) {
    const dateKey = extractDateKey(activity.startDateLocal);
    const existing = activitiesByDate.get(dateKey) ?? [];
    existing.push(activity);
    activitiesByDate.set(dateKey, existing);
  }

  for (const event of eventsSorted) {
    const dateKey = extractDateKey(event.startDateLocal);
    const existing = eventsByDate.get(dateKey) ?? [];
    existing.push(event);
    eventsByDate.set(dateKey, existing);

    const actualWorkoutActivityId = event.actualWorkout?.activityId;
    if (actualWorkoutActivityId) {
      const activityMatches = eventsByActualWorkoutActivityId.get(actualWorkoutActivityId) ?? [];
      activityMatches.push(event);
      eventsByActualWorkoutActivityId.set(actualWorkoutActivityId, activityMatches);
    }
  }

  const matchedActivityIds = new Set<string>();
  const matchedEventIds = new Set<number>();
  const activityEventMatches = new Map<string, IntervalEvent>();

  for (const activity of activitiesSorted) {
    const linkedEvent = (eventsByActualWorkoutActivityId.get(activity.id) ?? []).find(
      (event) => !matchedEventIds.has(event.id),
    );
    if (!linkedEvent || matchedActivityIds.has(activity.id)) {
      continue;
    }

    matchedActivityIds.add(activity.id);
    matchedEventIds.add(linkedEvent.id);
    activityEventMatches.set(activity.id, linkedEvent);
  }

  for (const activity of activitiesSorted) {
    const hintedEventId = inferEventIdHint(activity);
    if (!hintedEventId) {
      continue;
    }

    const hintedEvent = eventsSorted.find((event) => String(event.id) === hintedEventId);
    if (!hintedEvent || matchedActivityIds.has(activity.id) || matchedEventIds.has(hintedEvent.id)) {
      continue;
    }

    matchedActivityIds.add(activity.id);
    matchedEventIds.add(hintedEvent.id);
    activityEventMatches.set(activity.id, hintedEvent);
  }

  for (const event of eventsSorted) {
    if (matchedEventIds.has(event.id)) {
      continue;
    }

    const dateKey = extractDateKey(event.startDateLocal);
    const candidates = (activitiesByDate.get(dateKey) ?? []).filter((activity) => !matchedActivityIds.has(activity.id));
    const matchedActivity = chooseMatchedActivity(
      event,
      candidates,
      (eventsByDate.get(dateKey) ?? []).length,
      (activitiesByDate.get(dateKey) ?? []).length,
    );

    if (!matchedActivity) {
      continue;
    }

    matchedActivityIds.add(matchedActivity.id);
    matchedEventIds.add(event.id);
    activityEventMatches.set(matchedActivity.id, event);
  }

  const items: CoachWorkoutListItem[] = activitiesSorted.map((activity) => {
    const matchedEvent = activityEventMatches.get(activity.id) ?? null;
    const id = activity.id;

    return {
      id,
      source: 'activity',
      startDateLocal: activity.startDateLocal,
      event: matchedEvent,
      activity,
      summary: null,
      hasSummary: false,
      hasConversation: false,
    };
  });

  for (const event of eventsSorted) {
    if (matchedEventIds.has(event.id)) {
      continue;
    }

    items.push({
      id: String(event.id),
      source: 'event',
      startDateLocal: event.startDateLocal,
      event,
      activity: null,
      summary: null,
      hasSummary: false,
      hasConversation: false,
    });
  }

  return items.sort((left, right) => right.startDateLocal.localeCompare(left.startDateLocal));
}

function isSameDay(left: Date, right: Date): boolean {
  return left.getTime() === right.getTime();
}

function isWithinWeek(value: string, weekStart: Date): boolean {
  const weekStartKey = toDateKey(weekStart);
  const weekEndKey = toDateKey(addDays(weekStart, 6));
  const dateKey = extractDateKey(value);

  return dateKey >= weekStartKey && dateKey <= weekEndKey;
}

function withSummaryState(item: CoachWorkoutListItem, summary: WorkoutSummary | null): CoachWorkoutListItem {
  return {
    ...item,
    summary,
    hasSummary: summary !== null,
    hasConversation: summary?.messages.some((message) => message.role === 'coach') ?? false,
  };
}

function buildVisibleItems(
  allItems: CoachWorkoutListItem[],
  visibleWeekStart: Date,
  summaryCache: Map<string, WorkoutSummary>,
): CoachWorkoutListItem[] {
  const todayDateKey = toDateKey(new Date());

  return allItems
    .filter(
      (item) =>
        item.source === 'activity'
        && extractDateKey(item.startDateLocal) <= todayDateKey
        && isWithinWeek(item.startDateLocal, visibleWeekStart),
    )
    .map((item) => withSummaryState(item, summaryCache.get(item.id) ?? null));
}

function mergeSummaryCache(
  current: Map<string, WorkoutSummary>,
  summaries: WorkoutSummary[],
): Map<string, WorkoutSummary> {
  const next = new Map(current);

  for (const summary of summaries) {
    next.set(summary.workoutId, summary);
  }

  return next;
}

function replaceRequestedSummaries(
  current: Map<string, WorkoutSummary>,
  requestedWorkoutIds: string[],
  summaries: WorkoutSummary[],
): Map<string, WorkoutSummary> {
  const next = new Map(current);

  for (const workoutId of requestedWorkoutIds) {
    next.delete(workoutId);
  }

  for (const summary of summaries) {
    next.set(summary.workoutId, summary);
  }

  return next;
}

function chunkWorkoutIds(workoutIds: string[]): string[][] {
  const chunks: string[][] = [];

  for (let index = 0; index < workoutIds.length; index += MAX_SUMMARY_BATCH_SIZE) {
    chunks.push(workoutIds.slice(index, index + MAX_SUMMARY_BATCH_SIZE));
  }

  return chunks;
}

function defaultVisibleWeekStart(items: CoachWorkoutListItem[], currentWeekStart: Date): Date {
  if (items.some((item) => isWithinWeek(item.startDateLocal, currentWeekStart))) {
    return currentWeekStart;
  }

  const newestItem = items[0];
  return newestItem ? getMondayOfWeek(new Date(newestItem.startDateLocal)) : currentWeekStart;
}

export function useWorkoutList({ apiBaseUrl }: UseWorkoutListOptions): UseWorkoutListResult {
  const { getActivitiesForRange, error: contextError } = useCompletedWorkouts();
  const [currentWeekStart, setCurrentWeekStart] = useState(() => getMondayOfWeek(new Date()));
  const [visibleWeekStart, setVisibleWeekStart] = useState(() => getMondayOfWeek(new Date()));
  const [allItems, setAllItems] = useState<CoachWorkoutListItem[]>([]);
  const [summaryCache, setSummaryCache] = useState<Map<string, WorkoutSummary>>(() => new Map());
  const [loadedSummaryIds, setLoadedSummaryIds] = useState<Set<string>>(() => new Set());
  const [hasLoadedWorkouts, setHasLoadedWorkouts] = useState(false);
  const [state, setState] = useState<WorkoutListState>('loading');
  const [error, setError] = useState<string | null>(null);
  const currentWeekStartRef = useRef(currentWeekStart);
  const requestIdRef = useRef(0);
  const summaryRequestIdRef = useRef(0);
  const contextErrorRef = useRef(contextError);

  const items = useMemo(() => buildVisibleItems(allItems, visibleWeekStart, summaryCache), [
    allItems,
    visibleWeekStart,
    summaryCache,
  ]);

  useEffect(() => {
    contextErrorRef.current = contextError;
  }, [contextError]);

  const loadRecentWorkouts = useCallback(async () => {
    const requestId = requestIdRef.current + 1;
    requestIdRef.current = requestId;
    summaryRequestIdRef.current += 1;
    setHasLoadedWorkouts(false);
    setState('loading');
    setError(null);

    try {
      const latestCurrentWeekStart = getMondayOfWeek(new Date());
      const lookbackStart = addWeeks(latestCurrentWeekStart, -(WORKOUT_LOOKBACK_WEEKS - 1));
      const range = formatDateRange(lookbackStart, WORKOUT_LOOKBACK_WEEKS);
      const [events, activities] = await Promise.all([
        listEvents(apiBaseUrl, range),
        getActivitiesForRange(range.oldest, range.newest),
      ]);

      if (contextErrorRef.current?.kind === 'credentials-required') {
        setState('credentials-required');
        return;
      }

      if (contextErrorRef.current?.kind === 'network-error') {
        setState('error');
        setError(contextErrorRef.current.message);
        return;
      }

      const workoutEvents = [...events]
        .sort((left, right) => right.startDateLocal.localeCompare(left.startDateLocal))
        .slice(0, WORKOUT_LOOKBACK_WEEKS * WORKOUT_PAGE_SIZE);
      const recentActivities = [...activities]
        .sort((left, right) => right.startDateLocal.localeCompare(left.startDateLocal))
        .slice(0, WORKOUT_LOOKBACK_WEEKS * WORKOUT_PAGE_SIZE);
      const nextItems = buildWorkoutItems(workoutEvents, recentActivities);

      if (requestId !== requestIdRef.current) {
        return;
      }

      setAllItems(nextItems);
      setLoadedSummaryIds(new Set());
      const nextVisibleWeekStart = defaultVisibleWeekStart(nextItems, latestCurrentWeekStart);
      const previousCurrentWeekStart = currentWeekStartRef.current;
      currentWeekStartRef.current = latestCurrentWeekStart;
      setCurrentWeekStart(latestCurrentWeekStart);
      setVisibleWeekStart((current) => {
        if (isSameDay(current, previousCurrentWeekStart) || current > latestCurrentWeekStart) {
          return nextVisibleWeekStart;
        }

        return current;
      });
      setHasLoadedWorkouts(true);
    } catch (loadError) {
      if (requestId !== requestIdRef.current) {
        return;
      }

      if (loadError instanceof AuthenticationError) {
        window.location.href = '/';
        return;
      }

      if (loadError instanceof HttpError && loadError.status === 422) {
        setState('credentials-required');
        return;
      }

      setState('error');
      setError(loadError instanceof Error ? loadError.message : 'Unknown error');
    }
  }, [apiBaseUrl, getActivitiesForRange]);

  useEffect(() => {
    void loadRecentWorkouts();
  }, [loadRecentWorkouts]);

  useEffect(() => {
    if (!hasLoadedWorkouts) {
      return;
    }

    const summaryTargetIds = items.map((item) => item.id);

    if (summaryTargetIds.length === 0) {
      setState('ready');
      setError(null);
      return;
    }

    const missingSummaryIds = summaryTargetIds.filter((workoutId) => !loadedSummaryIds.has(workoutId));

    if (missingSummaryIds.length === 0) {
      setState('ready');
      setError(null);
      return;
    }

    const requestId = summaryRequestIdRef.current + 1;
    summaryRequestIdRef.current = requestId;
    setState('loading');
    setError(null);

    void (async () => {
      try {
        const summaries = (
          await Promise.all(
            chunkWorkoutIds(missingSummaryIds).map((workoutIds) => listWorkoutSummaries(apiBaseUrl, workoutIds)),
          )
        ).flat();

        if (requestId !== summaryRequestIdRef.current) {
          return;
        }

        setSummaryCache((current) => replaceRequestedSummaries(current, missingSummaryIds, summaries));
        setLoadedSummaryIds((current) => {
          const next = new Set(current);

          for (const workoutId of missingSummaryIds) {
            next.add(workoutId);
          }

          return next;
        });
        setState('ready');
      } catch (loadError) {
        if (requestId !== summaryRequestIdRef.current) {
          return;
        }

        if (loadError instanceof AuthenticationError) {
          window.location.href = '/';
          return;
        }

        if (loadError instanceof HttpError && loadError.status === 422) {
          setState('credentials-required');
          return;
        }

        setState('error');
        setError(loadError instanceof Error ? loadError.message : 'Unknown error');
      }
    })();
  }, [apiBaseUrl, hasLoadedWorkouts, items, loadedSummaryIds]);

  const weekLabel = useMemo(() => {
    return formatRangeLabel(visibleWeekStart, addDays(visibleWeekStart, 6));
  }, [visibleWeekStart]);
  const canGoToNewerWeek = visibleWeekStart < currentWeekStart;

  const replaceSummary = useCallback((summary: WorkoutSummary) => {
    setSummaryCache((current) => mergeSummaryCache(current, [summary]));
    setLoadedSummaryIds((current) => {
      const next = new Set(current);
      next.add(summary.workoutId);
      return next;
    });
  }, []);

  return {
    items,
    state,
    error,
    weekLabel,
    canGoToNewerWeek,
    goToOlderWeek: () => {
      setVisibleWeekStart((current) => addWeeks(current, -1));
    },
    goToNewerWeek: () => {
      setVisibleWeekStart((current) => {
        const next = addWeeks(current, 1);
        return next > currentWeekStartRef.current ? currentWeekStartRef.current : next;
      });
    },
    refresh: loadRecentWorkouts,
    replaceSummary,
  };
}
