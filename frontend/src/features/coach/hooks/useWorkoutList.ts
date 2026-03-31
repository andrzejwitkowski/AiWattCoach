import { useCallback, useEffect, useMemo, useState } from 'react';

import { listEvents } from '../../intervals/api/intervals';
import { AuthenticationError, HttpError } from '../../../lib/httpClient';
import { addDays, addWeeks, formatDateRange, getMondayOfWeek } from '../../calendar/utils/dateUtils';
import { listWorkoutSummaries } from '../api/workoutSummary';
import type { CoachWorkoutListItem } from '../types';

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
      const events = await listEvents(apiBaseUrl, formatDateRange(lookbackStart, WORKOUT_LOOKBACK_WEEKS));
      const workoutEvents = events
        .sort((left, right) => right.startDateLocal.localeCompare(left.startDateLocal))
        .slice(0, WORKOUT_LOOKBACK_WEEKS * WORKOUT_PAGE_SIZE);
      const summaries = await listWorkoutSummaries(
        apiBaseUrl,
        workoutEvents.map((event) => String(event.id)),
      );
      const summariesByEventId = new Map(summaries.map((summary) => [summary.eventId, summary]));

      const nextItems = workoutEvents.map((event) => {
          const summary = summariesByEventId.get(String(event.id)) ?? null;
          return {
            event,
            summary,
            hasSummary: summary !== null,
            hasConversation: summary?.messages.some((message) => message.role === 'coach') ?? false,
          };
        });

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

    const startDate = new Date(items[items.length - 1].event.startDateLocal);
    const endDate = new Date(items[0].event.startDateLocal);
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
