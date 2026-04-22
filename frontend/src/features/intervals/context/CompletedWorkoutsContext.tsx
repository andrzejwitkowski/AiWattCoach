import { createContext, useCallback, useContext, useRef, useState } from 'react';

import { listActivities } from '../api/intervals';
import { AuthenticationError, HttpError } from '../../../lib/httpClient';
import type { IntervalActivity } from '../types';

const CACHE_TTL_MS = 5 * 60 * 1000;

type CachedRange = {
  activities: IntervalActivity[];
  loadedAt: number;
};

type InflightRequest = Promise<IntervalActivity[]>;

type CompletedWorkoutsContextValue = {
  getActivitiesForRange: (oldest: string, newest: string) => Promise<IntervalActivity[]>;
  invalidateRange: (oldest: string, newest: string) => void;
  invalidateAll: () => void;
  isLoading: boolean;
  error: string | null;
};

const CompletedWorkoutsContext = createContext<CompletedWorkoutsContextValue | null>(null);

function buildCacheKey(oldest: string, newest: string): string {
  return `${oldest}|${newest}`;
}

function isStale(loadedAt: number): boolean {
  return Date.now() - loadedAt > CACHE_TTL_MS;
}

export function CompletedWorkoutsProvider({
  children,
  apiBaseUrl,
}: {
  children: React.ReactNode;
  apiBaseUrl: string;
}) {
  const cacheRef = useRef<Map<string, CachedRange>>(new Map());
  const inflightRef = useRef<Map<string, InflightRequest>>(new Map());
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const fetchRange = useCallback(async (oldest: string, newest: string): Promise<IntervalActivity[]> => {
    const key = buildCacheKey(oldest, newest);
    const cached = cacheRef.current.get(key);

    if (cached && !isStale(cached.loadedAt)) {
      return cached.activities;
    }

    const existingInflight = inflightRef.current.get(key);
    if (existingInflight) {
      return existingInflight;
    }

    setIsLoading(true);
    setError(null);

    const promise = listActivities(apiBaseUrl, { oldest, newest })
      .then((activities) => {
        cacheRef.current.set(key, { activities, loadedAt: Date.now() });
        return activities;
      })
      .catch((err) => {
        if (err instanceof AuthenticationError) {
          window.location.href = '/';
          return [] as IntervalActivity[];
        }

        if (err instanceof HttpError && err.status === 422) {
          setError('credentials-required');
        } else {
          setError(err instanceof Error ? err.message : 'Failed to load completed workouts');
        }

        return [] as IntervalActivity[];
      })
      .finally(() => {
        inflightRef.current.delete(key);
        setIsLoading(false);
      });

    inflightRef.current.set(key, promise);
    return promise;
  }, [apiBaseUrl]);

  const getActivitiesForRange = useCallback(async (oldest: string, newest: string): Promise<IntervalActivity[]> => {
    const key = buildCacheKey(oldest, newest);
    const cached = cacheRef.current.get(key);

    if (cached && !isStale(cached.loadedAt)) {
      return cached.activities;
    }

    if (cached && isStale(cached.loadedAt)) {
      void fetchRange(oldest, newest);
      return cached.activities;
    }

    return fetchRange(oldest, newest);
  }, [fetchRange]);

  const invalidateRange = useCallback((oldest: string, newest: string) => {
    const key = buildCacheKey(oldest, newest);
    cacheRef.current.delete(key);
  }, []);

  const invalidateAll = useCallback(() => {
    cacheRef.current.clear();
  }, []);

  return (
    <CompletedWorkoutsContext.Provider value={{ getActivitiesForRange, invalidateRange, invalidateAll, isLoading, error }}>
      {children}
    </CompletedWorkoutsContext.Provider>
  );
}

export function useCompletedWorkouts() {
  const context = useContext(CompletedWorkoutsContext);
  if (!context) {
    throw new Error('useCompletedWorkouts must be used within a CompletedWorkoutsProvider');
  }
  return context;
}
