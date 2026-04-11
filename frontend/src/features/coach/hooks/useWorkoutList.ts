import { useCallback, useEffect, useMemo, useRef, useState } from 'react';

import { listActivities, listEvents } from '../../intervals/api/intervals';
import { AuthenticationError, HttpError } from '../../../lib/httpClient';
import { addDays, addWeeks, extractDateKey, formatDateRange, getMondayOfWeek, toDateKey } from '../../calendar/utils/dateUtils';
import { listWorkoutSummaries } from '../api/workoutSummary';
import type { CoachWorkoutListItem, WorkoutSummary } from '../types';
import type { IntervalActivity, IntervalEvent } from '../../intervals/types';

export type WorkoutListState = 'loading' | 'ready' | 'error' | 'credentials-required';

const WORKOUT_PAGE_SIZE = 7;
const WORKOUT_LOOKBACK_WEEKS = 12;

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

function chooseSummary(candidates: WorkoutSummary[]): WorkoutSummary | null {
  if (candidates.length === 0) {
    return null;
  }

  return [...candidates].sort((left, right) => {
    return right.updatedAtEpochSeconds - left.updatedAtEpochSeconds
      || right.createdAtEpochSeconds - left.createdAtEpochSeconds;
  })[0] ?? null;
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
  summaries: WorkoutSummary[],
): CoachWorkoutListItem[] {
  const eventsSorted = [...events].sort((left, right) => right.startDateLocal.localeCompare(left.startDateLocal));
  const activitiesSorted = [...activities].sort((left, right) => right.startDateLocal.localeCompare(left.startDateLocal));
  const summariesById = new Map(summaries.map((summary) => [summary.workoutId, summary]));
  const activitiesByDate = new Map<string, IntervalActivity[]>();
  const eventsByDate = new Map<string, IntervalEvent[]>();

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
  }

  const matchedActivityIds = new Set<string>();
  const matchedEventIds = new Set<number>();
  const activityEventMatches = new Map<string, IntervalEvent>();

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
    const summary = chooseSummary(
      [summariesById.get(activity.id), matchedEvent ? summariesById.get(String(matchedEvent.id)) : undefined].filter(
        (value): value is WorkoutSummary => value !== undefined,
      ),
    );
    const id = summary?.workoutId ?? activity.id;

    return {
      id,
      source: 'activity',
      startDateLocal: activity.startDateLocal,
      event: matchedEvent,
      activity,
      summary,
      hasSummary: summary !== null,
      hasConversation: summary?.messages.some((message) => message.role === 'coach') ?? false,
    };
  });

  for (const event of eventsSorted) {
    if (matchedEventIds.has(event.id)) {
      continue;
    }

    const summary = summariesById.get(String(event.id)) ?? null;
    items.push({
      id: summary?.workoutId ?? String(event.id),
      source: 'event',
      startDateLocal: event.startDateLocal,
      event,
      activity: null,
      summary,
      hasSummary: summary !== null,
      hasConversation: summary?.messages.some((message) => message.role === 'coach') ?? false,
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

function applySummaryToItem(item: CoachWorkoutListItem, summary: WorkoutSummary): CoachWorkoutListItem {
  const hasSummaryMatch = item.id === summary.workoutId
    || item.summary?.workoutId === summary.workoutId
    || item.activity?.id === summary.workoutId
    || String(item.event?.id ?? '') === summary.workoutId;

  if (!hasSummaryMatch) {
    return item;
  }

  return {
    ...item,
    id: summary.workoutId,
    summary,
    hasSummary: true,
    hasConversation: summary.messages.some((message) => message.role === 'coach'),
  };
}

function defaultVisibleWeekStart(items: CoachWorkoutListItem[], currentWeekStart: Date): Date {
  if (items.some((item) => isWithinWeek(item.startDateLocal, currentWeekStart))) {
    return currentWeekStart;
  }

  const newestItem = items[0];
  return newestItem ? getMondayOfWeek(new Date(newestItem.startDateLocal)) : currentWeekStart;
}

export function useWorkoutList({ apiBaseUrl }: UseWorkoutListOptions): UseWorkoutListResult {
  const [currentWeekStart, setCurrentWeekStart] = useState(() => getMondayOfWeek(new Date()));
  const [visibleWeekStart, setVisibleWeekStart] = useState(() => getMondayOfWeek(new Date()));
  const [allItems, setAllItems] = useState<CoachWorkoutListItem[]>([]);
  const [items, setItems] = useState<CoachWorkoutListItem[]>([]);
  const [state, setState] = useState<WorkoutListState>('loading');
  const [error, setError] = useState<string | null>(null);
  const currentWeekStartRef = useRef(currentWeekStart);
  const requestIdRef = useRef(0);

  const loadRecentWorkouts = useCallback(async () => {
    const requestId = requestIdRef.current + 1;
    requestIdRef.current = requestId;
    setState('loading');
    setError(null);

    try {
      const latestCurrentWeekStart = getMondayOfWeek(new Date());
      const lookbackStart = addWeeks(latestCurrentWeekStart, -(WORKOUT_LOOKBACK_WEEKS - 1));
      const range = formatDateRange(lookbackStart, WORKOUT_LOOKBACK_WEEKS);
      const [events, activities] = await Promise.all([
        listEvents(apiBaseUrl, range),
        listActivities(apiBaseUrl, range),
      ]);
      const workoutEvents = [...events]
        .sort((left, right) => right.startDateLocal.localeCompare(left.startDateLocal))
        .slice(0, WORKOUT_LOOKBACK_WEEKS * WORKOUT_PAGE_SIZE);
      const recentActivities = [...activities]
        .sort((left, right) => right.startDateLocal.localeCompare(left.startDateLocal))
        .slice(0, WORKOUT_LOOKBACK_WEEKS * WORKOUT_PAGE_SIZE);
      const summaries = await listWorkoutSummaries(
        apiBaseUrl,
        [
          ...workoutEvents.map((event) => String(event.id)),
          ...recentActivities.map((activity) => activity.id),
        ],
      );
      const nextItems = buildWorkoutItems(workoutEvents, recentActivities, summaries);

      if (requestId !== requestIdRef.current) {
        return;
      }

      setAllItems(nextItems);
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
      setState('ready');
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
  }, [apiBaseUrl]);

  useEffect(() => {
    void loadRecentWorkouts();
  }, [loadRecentWorkouts]);

  useEffect(() => {
    const todayDateKey = toDateKey(new Date());
    setItems(
      allItems.filter(
        (item) =>
          item.source === 'activity'
          && extractDateKey(item.startDateLocal) <= todayDateKey
          && isWithinWeek(item.startDateLocal, visibleWeekStart),
      ),
    );
  }, [allItems, visibleWeekStart]);

  const weekLabel = useMemo(() => {
    return formatRangeLabel(visibleWeekStart, addDays(visibleWeekStart, 6));
  }, [visibleWeekStart]);
  const canGoToNewerWeek = visibleWeekStart < currentWeekStart;

  const replaceSummary = useCallback((summary: WorkoutSummary) => {
    setAllItems((current) => current.map((item) => applySummaryToItem(item, summary)));
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
