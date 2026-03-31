import { useCallback, useEffect, useMemo, useState } from 'react';

import type { IntervalEvent } from '../../intervals/types';
import { listEvents } from '../../intervals/api/intervals';
import { AuthenticationError, HttpError } from '../../../lib/httpClient';
import { addDays, addWeeks, formatDateRange, getMondayOfWeek } from '../../calendar/utils/dateUtils';
import { listWorkoutSummaries } from '../api/workoutSummary';
import type { CoachWorkoutListItem } from '../types';

export type WorkoutListState = 'loading' | 'ready' | 'error' | 'credentials-required';

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

function isCoachEligibleEvent(event: IntervalEvent): boolean {
  return event.actualWorkout !== null || event.category === 'WORKOUT' || event.category === 'RACE';
}

function formatWeekLabel(startOfWeek: Date): string {
  const endOfWeek = addDays(startOfWeek, 6);
  const formatter = new Intl.DateTimeFormat(undefined, {
    month: 'short',
    day: 'numeric',
  });
  return `${formatter.format(startOfWeek)} - ${formatter.format(endOfWeek)}`;
}

export function useWorkoutList({ apiBaseUrl }: UseWorkoutListOptions): UseWorkoutListResult {
  const currentWeekStart = useMemo(() => getMondayOfWeek(new Date()), []);
  const [weekStart, setWeekStart] = useState<Date>(currentWeekStart);
  const [items, setItems] = useState<CoachWorkoutListItem[]>([]);
  const [state, setState] = useState<WorkoutListState>('loading');
  const [error, setError] = useState<string | null>(null);

  const loadWeek = useCallback(async () => {
    setState('loading');
    setError(null);

    try {
      const events = await listEvents(apiBaseUrl, formatDateRange(weekStart, 1));
      const workoutEvents = events
        .filter(isCoachEligibleEvent)
        .sort((left, right) => right.startDateLocal.localeCompare(left.startDateLocal))
        .slice(0, 7);
      const summaries = await listWorkoutSummaries(
        apiBaseUrl,
        workoutEvents.map((event) => String(event.id)),
      );
      const summariesByEventId = new Map(summaries.map((summary) => [summary.eventId, summary]));

      setItems(
        workoutEvents.map((event) => {
          const summary = summariesByEventId.get(String(event.id)) ?? null;
          return {
            event,
            summary,
            hasSummary: summary !== null,
            hasConversation: summary?.messages.some((message) => message.role === 'coach') ?? false,
          };
        }),
      );
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
  }, [apiBaseUrl, weekStart]);

  useEffect(() => {
    void loadWeek();
  }, [loadWeek]);

  const weekLabel = useMemo(() => formatWeekLabel(weekStart), [weekStart]);
  const canGoToNewerWeek = weekStart.getTime() < currentWeekStart.getTime();

  return {
    items,
    state,
    error,
    weekLabel,
    canGoToNewerWeek,
    goToOlderWeek: () => {
      setWeekStart((current) => addWeeks(current, -1));
    },
    goToNewerWeek: () => {
      setWeekStart((current) => {
        const next = addWeeks(current, 1);
        return next.getTime() > currentWeekStart.getTime() ? currentWeekStart : next;
      });
    },
    refresh: loadWeek,
  };
}
