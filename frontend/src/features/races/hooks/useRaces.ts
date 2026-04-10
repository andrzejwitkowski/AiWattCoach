import { useCallback, useEffect, useMemo, useState } from 'react';

import { listRaces } from '../api/races';
import type { Race } from '../types';
import { splitRacesByDate, toDateKey } from '../utils';

type UseRacesOptions = {
  apiBaseUrl: string;
};

type UseRacesResult = {
  races: Race[];
  upcomingRaces: Race[];
  completedRaces: Race[];
  isLoading: boolean;
  error: string | null;
  refresh: () => Promise<void>;
};

const PAST_DAYS = 365;
const FUTURE_DAYS = 365;

function addDays(date: Date, days: number): Date {
  const next = new Date(date);
  next.setDate(next.getDate() + days);
  return next;
}

export function useRaces({ apiBaseUrl }: UseRacesOptions): UseRacesResult {
  const [races, setRaces] = useState<Race[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    setIsLoading(true);
    setError(null);

    try {
      const today = new Date();
      const data = await listRaces(apiBaseUrl, {
        oldest: toDateKey(addDays(today, -PAST_DAYS)),
        newest: toDateKey(addDays(today, FUTURE_DAYS)),
      });
      setRaces(data);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to load races');
    } finally {
      setIsLoading(false);
    }
  }, [apiBaseUrl]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const { upcoming, completed } = useMemo(() => splitRacesByDate(races, toDateKey(new Date())), [races]);

  return {
    races,
    upcomingRaces: upcoming,
    completedRaces: completed,
    isLoading,
    error,
    refresh,
  };
}
