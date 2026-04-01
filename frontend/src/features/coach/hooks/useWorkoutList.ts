import { useCallback, useEffect, useMemo, useState } from 'react';

import { listActivities, listEvents } from '../../intervals/api/intervals';
import { AuthenticationError, HttpError } from '../../../lib/httpClient';
import { addDays, addWeeks, extractDateKey, formatDateRange, getMondayOfWeek } from '../../calendar/utils/dateUtils';
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

  for (const event of eventsSorted) {
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
    const hintedEventId = inferEventIdHint(activity);
    const hintedEvent = hintedEventId
      ? eventsSorted.find((event) => String(event.id) === hintedEventId) ?? null
      : null;
    const matchedEvent = activityEventMatches.get(activity.id) ?? hintedEvent;
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

export function useWorkoutList({ apiBaseUrl }: UseWorkoutListOptions): UseWorkoutListResult {
  const currentWeekStart = useMemo(() => getMondayOfWeek(new Date()), []);
  const [pageIndex, setPageIndex] = useState(0);
  const [allItems, setAllItems] = useState<CoachWorkoutListItem[]>([]);
  const [items, setItems] = useState<CoachWorkoutListItem[]>([]);
  const [state, setState] = useState<WorkoutListState>('loading');
  const [error, setError] = useState<string | null>(null);

  const loadRecentWorkouts = useCallback(async () => {
    setState('loading');
    setError(null);

    try {
      const lookbackStart = addWeeks(currentWeekStart, -(WORKOUT_LOOKBACK_WEEKS - 1));
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

      setAllItems(nextItems);
      setState('ready');
    } catch (loadError) {
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
  }, [apiBaseUrl, currentWeekStart]);

  useEffect(() => {
    void loadRecentWorkouts();
  }, [loadRecentWorkouts]);

  useEffect(() => {
    const maxPageIndex = Math.max(0, Math.ceil(allItems.length / WORKOUT_PAGE_SIZE) - 1);
    setPageIndex((current) => Math.min(current, maxPageIndex));
  }, [allItems.length]);

  useEffect(() => {
    const start = pageIndex * WORKOUT_PAGE_SIZE;
    setItems(allItems.slice(start, start + WORKOUT_PAGE_SIZE));
  }, [allItems, pageIndex]);

  const maxPageIndex = Math.max(0, Math.ceil(allItems.length / WORKOUT_PAGE_SIZE) - 1);
  const weekLabel = useMemo(() => {
    if (items.length === 0) {
      return formatRangeLabel(addDays(currentWeekStart, -6), currentWeekStart);
    }

    const startDate = new Date(items[items.length - 1].startDateLocal);
    const endDate = new Date(items[0].startDateLocal);
    return formatRangeLabel(startDate, endDate);
  }, [currentWeekStart, items]);
  const canGoToNewerWeek = pageIndex > 0;

  return {
    items,
    state,
    error,
    weekLabel,
    canGoToNewerWeek,
    goToOlderWeek: () => {
      setPageIndex((current) => Math.min(current + 1, maxPageIndex));
    },
    goToNewerWeek: () => {
      setPageIndex((current) => Math.max(current - 1, 0));
    },
    refresh: loadRecentWorkouts,
  };
}
