import { useCallback, useEffect, useMemo, useRef, useState } from 'react';

import { usePlannedRestDaysApi } from '../api/plannedRestDays';
import type { PlannedRestDay } from '../types';
import { splitPlannedRestDaysByDate, toDateKey } from '../utils';

type UsePlannedRestDaysResult = {
  entries: PlannedRestDay[];
  upcomingEntries: PlannedRestDay[];
  pastEntries: PlannedRestDay[];
  isLoading: boolean;
  error: string | null;
  refresh: () => Promise<void>;
};

const PAST_DAYS = 365;
const FUTURE_DAYS = 400;

function addDays(date: Date, days: number): Date {
  const next = new Date(date);
  next.setDate(next.getDate() + days);
  return next;
}

export function usePlannedRestDays(): UsePlannedRestDaysResult {
  const { listPlannedRestDays } = usePlannedRestDaysApi();
  const latestRequestIdRef = useRef(0);
  const [entries, setEntries] = useState<PlannedRestDay[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [todayKey, setTodayKey] = useState(() => toDateKey(new Date()));

  const refresh = useCallback(async () => {
    const requestId = ++latestRequestIdRef.current;
    setIsLoading(true);
    setError(null);

    try {
      const today = new Date();
      const data = await listPlannedRestDays({
        oldest: toDateKey(addDays(today, -PAST_DAYS)),
        newest: toDateKey(addDays(today, FUTURE_DAYS)),
      });
      if (requestId !== latestRequestIdRef.current) {
        return;
      }
      setEntries(data);
    } catch (err) {
      if (requestId !== latestRequestIdRef.current) {
        return;
      }
      setError(err instanceof Error ? err.message : 'Failed to load planned rest days');
    } finally {
      if (requestId === latestRequestIdRef.current) {
        setIsLoading(false);
      }
    }
  }, [listPlannedRestDays]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  useEffect(() => {
    const now = new Date();
    const nextMidnight = new Date(now);
    nextMidnight.setHours(24, 0, 0, 0);

    const timeout = window.setTimeout(() => {
      setTodayKey(toDateKey(new Date()));
    }, nextMidnight.getTime() - now.getTime());

    return () => window.clearTimeout(timeout);
  }, [todayKey]);

  const { upcoming, past } = useMemo(
    () => splitPlannedRestDaysByDate(entries, todayKey),
    [entries, todayKey],
  );

  return {
    entries,
    upcomingEntries: upcoming,
    pastEntries: past,
    isLoading,
    error,
    refresh,
  };
}
