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

export type CompletedWorkoutsError =
  | { kind: 'credentials-required' }
  | { kind: 'network-error'; message: string };

type CompletedWorkoutsContextValue = {
  getActivitiesForRange: (oldest: string, newest: string) => Promise<IntervalActivity[]>;
  invalidateRange: (oldest: string, newest: string) => void;
  invalidateAll: () => void;
  isLoading: boolean;
  error: CompletedWorkoutsError | null;
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
  const invalidationRef = useRef<Map<string, number>>(new Map());
  const [inflightCount, setInflightCount] = useState(0);
  const [error, setError] = useState<CompletedWorkoutsError | null>(null);

  const fetchRange = useCallback(async (oldest: string, newest: string): Promise<IntervalActivity[]> => {
    const key = buildCacheKey(oldest, newest);
    const cached = cacheRef.current.get(key);

    if (cached && !isStale(cached.loadedAt)) {
      setError(null);
      return cached.activities;
    }

    const existingInflight = inflightRef.current.get(key);
    if (existingInflight) {
      return existingInflight;
    }

    const invalidationToken = (invalidationRef.current.get(key) ?? 0) + 1;
    invalidationRef.current.set(key, invalidationToken);

    setInflightCount((c) => c + 1);

    const promise = listActivities(apiBaseUrl, { oldest, newest })
      .then((activities) => {
        if (invalidationRef.current.get(key) !== invalidationToken) {
          return [] as IntervalActivity[];
        }

        cacheRef.current.set(key, { activities, loadedAt: Date.now() });
        setError(null);
        return activities;
      })
      .catch((err) => {
        if (err instanceof AuthenticationError) {
          window.location.href = '/';
          throw err;
        }

        const completedError: CompletedWorkoutsError = err instanceof HttpError && err.status === 422
          ? { kind: 'credentials-required' }
          : { kind: 'network-error', message: err instanceof Error ? err.message : 'Failed to load completed workouts' };

        if (invalidationRef.current.get(key) === invalidationToken) {
          setError(completedError);
        }

        throw err;
      })
      .finally(() => {
        inflightRef.current.delete(key);
        setInflightCount((c) => c - 1);
      });

    inflightRef.current.set(key, promise);
    return promise;
  }, [apiBaseUrl]);

  const getActivitiesForRange = useCallback(async (oldest: string, newest: string): Promise<IntervalActivity[]> => {
    const key = buildCacheKey(oldest, newest);
    const cached = cacheRef.current.get(key);

    if (cached && !isStale(cached.loadedAt)) {
      setError(null);
      return cached.activities;
    }

    if (cached && isStale(cached.loadedAt)) {
      void fetchRange(oldest, newest).catch(() => {});
      return cached.activities;
    }

    return fetchRange(oldest, newest);
  }, [fetchRange]);

  const invalidateRange = useCallback((oldest: string, newest: string) => {
    const key = buildCacheKey(oldest, newest);
    cacheRef.current.delete(key);
    const current = invalidationRef.current.get(key) ?? 0;
    invalidationRef.current.set(key, current + 1);
  }, []);

  const invalidateAll = useCallback(() => {
    cacheRef.current.clear();
    const nextToken = Date.now();
    invalidationRef.current.clear();
    invalidationRef.current.set('__global__', nextToken);
  }, []);

  return (
    <CompletedWorkoutsContext.Provider value={{ getActivitiesForRange, invalidateRange, invalidateAll, isLoading: inflightCount > 0, error }}>
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

export function __resetCachesForTesting() {
}
