import { createContext, useCallback, useContext, useMemo, useRef } from 'react';

import type { WorkoutSummary } from '../types';

type CoachSessionCacheValue = {
  getSummary: (workoutId: string) => WorkoutSummary | undefined;
  hasSummary: (workoutId: string) => boolean;
  setSummary: (summary: WorkoutSummary) => void;
};

const CoachSessionCacheContext = createContext<CoachSessionCacheValue | null>(null);

export function CoachSessionCacheProvider({ children }: { children: React.ReactNode }) {
  const summariesRef = useRef<Map<string, WorkoutSummary>>(new Map());

  const getSummary = useCallback((workoutId: string) => summariesRef.current.get(workoutId), []);

  const hasSummary = useCallback(
    (workoutId: string) => summariesRef.current.has(workoutId),
    [],
  );

  const setSummary = useCallback((summary: WorkoutSummary) => {
    summariesRef.current.set(summary.workoutId, summary);
  }, []);

  const value = useMemo(
    () => ({
      getSummary,
      hasSummary,
      setSummary,
    }),
    [getSummary, hasSummary, setSummary],
  );

  return (
    <CoachSessionCacheContext.Provider value={value}>{children}</CoachSessionCacheContext.Provider>
  );
}

export function useCoachSessionCache(): CoachSessionCacheValue {
  const context = useContext(CoachSessionCacheContext);
  if (!context) {
    throw new Error('useCoachSessionCache must be used within CoachSessionCacheProvider');
  }
  return context;
}
