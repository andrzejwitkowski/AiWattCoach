import { useCallback, useEffect, useMemo, useRef, useState } from 'react';

import { listActivities, listEvents } from '../../intervals/api/intervals';
import { AuthenticationError, HttpError } from '../../../lib/httpClient';
import { addDays, addWeeks, formatDateRange, getMondayOfWeek } from '../../calendar/utils/dateUtils';
import { listWorkoutSummaries, type WorkoutSummaryDateRange } from '../api/workoutSummary';
import { useCoachSessionCache } from '../context/CoachSessionCache';
import type { CoachWorkoutListItem, WorkoutSummary } from '../types';
import {
  buildVisibleItems,
  buildWorkoutItems,
  chunkWorkoutIds,
  defaultVisibleWeekStart,
  formatRangeLabel,
  isSameDay,
  weekDateRange,
} from './coachWorkoutListItems';

export type WorkoutListState = 'loading' | 'ready' | 'error' | 'credentials-required';
export type WorkoutSummariesState = 'idle' | 'loading' | 'ready' | 'error';

const WORKOUT_PAGE_SIZE = 7;
const WORKOUT_LOOKBACK_WEEKS = 12;
const MAX_SUMMARY_BATCH_SIZE = 31;

type UseWorkoutListOptions = {
  apiBaseUrl: string;
};

type UseWorkoutListResult = {
  items: CoachWorkoutListItem[];
  state: WorkoutListState;
  summariesState: WorkoutSummariesState;
  error: string | null;
  weekLabel: string;
  visibleWeekRange: WorkoutSummaryDateRange;
  canGoToNewerWeek: boolean;
  goToOlderWeek: () => void;
  goToNewerWeek: () => void;
  refresh: () => Promise<void>;
  replaceSummary: (summary: WorkoutSummary) => void;
};

export function useWorkoutList({ apiBaseUrl }: UseWorkoutListOptions): UseWorkoutListResult {
  const { clearSummaries, getSummary, hydrateMetadataSummaries, revision, upsertFullSummary } = useCoachSessionCache();
  const [currentWeekStart, setCurrentWeekStart] = useState(() => getMondayOfWeek(new Date()));
  const [visibleWeekStart, setVisibleWeekStart] = useState(() => getMondayOfWeek(new Date()));
  const [allItems, setAllItems] = useState<CoachWorkoutListItem[]>([]);
  const [loadedSummaryIds, setLoadedSummaryIds] = useState<Set<string>>(() => new Set());
  const [hasLoadedWorkouts, setHasLoadedWorkouts] = useState(false);
  const [state, setState] = useState<WorkoutListState>('loading');
  const [summariesState, setSummariesState] = useState<WorkoutSummariesState>('idle');
  const [error, setError] = useState<string | null>(null);
  const currentWeekStartRef = useRef(currentWeekStart);
  const requestIdRef = useRef(0);
  const summaryRequestIdRef = useRef(0);

  const visibleWeekRange = useMemo(() => weekDateRange(visibleWeekStart), [visibleWeekStart]);

  const items = useMemo(() => buildVisibleItems(allItems, visibleWeekStart, getSummary), [
    allItems,
    getSummary,
    revision,
    visibleWeekStart,
  ]);

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
        listActivities(apiBaseUrl, { ...range, detail: 'list' }),
      ]);

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
    if (!hasLoadedWorkouts) {
      return;
    }

    const summaryTargetIds = items.map((item) => item.id);

    if (summaryTargetIds.length === 0) {
      setSummariesState('ready');
      return;
    }

    const missingSummaryIds = summaryTargetIds.filter((workoutId) => !loadedSummaryIds.has(workoutId));

    if (missingSummaryIds.length === 0) {
      setSummariesState('ready');
      return;
    }

    const requestId = summaryRequestIdRef.current + 1;
    summaryRequestIdRef.current = requestId;
    setSummariesState('loading');
    const summariesAtRequestStart = new Map(
      missingSummaryIds.map((workoutId) => [workoutId, getSummary(workoutId)]),
    );

    void (async () => {
      try {
        const summaries = (
          await Promise.all(
            chunkWorkoutIds(missingSummaryIds, MAX_SUMMARY_BATCH_SIZE).map((workoutIds) =>
              listWorkoutSummaries(apiBaseUrl, workoutIds, {
                range: visibleWeekRange,
                view: 'metadata',
              }),
            ),
          )
        ).flat();

        if (requestId !== summaryRequestIdRef.current) {
          return;
        }

        const returnedSummaryIds = new Set(summaries.map((summary) => summary.workoutId));
        const omittedSummaryIds = missingSummaryIds.filter(
          (workoutId) => !returnedSummaryIds.has(workoutId)
            && getSummary(workoutId) === summariesAtRequestStart.get(workoutId),
        );

        hydrateMetadataSummaries(missingSummaryIds, summaries);
        if (omittedSummaryIds.length > 0) {
          clearSummaries(omittedSummaryIds);
        }
        setLoadedSummaryIds((current) => {
          const next = new Set(current);

          for (const workoutId of missingSummaryIds) {
            next.add(workoutId);
          }

          return next;
        });
        setSummariesState('ready');
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

        setSummariesState('error');
        setError(loadError instanceof Error ? loadError.message : 'Unknown error');
      }
    })();
  }, [apiBaseUrl, clearSummaries, getSummary, hasLoadedWorkouts, hydrateMetadataSummaries, items, loadedSummaryIds, visibleWeekRange]);

  const weekLabel = useMemo(() => {
    return formatRangeLabel(visibleWeekStart, addDays(visibleWeekStart, 6));
  }, [visibleWeekStart]);
  const canGoToNewerWeek = visibleWeekStart < currentWeekStart;

  const replaceSummary = useCallback((summary: WorkoutSummary) => {
    upsertFullSummary(summary);
    setLoadedSummaryIds((current) => {
      const next = new Set(current);
      next.add(summary.workoutId);
      return next;
    });
  }, [upsertFullSummary]);

  return {
    items,
    state,
    summariesState,
    error,
    weekLabel,
    visibleWeekRange,
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
