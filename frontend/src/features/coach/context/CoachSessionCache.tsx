import { createContext, useCallback, useContext, useMemo, useRef, useState } from 'react';

import type { WorkoutSummary } from '../types';

function mergeMetadataIntoSummary(
  existing: WorkoutSummary | undefined,
  incoming: WorkoutSummary,
): WorkoutSummary {
  if (!existing) {
    return incoming;
  }

  if (incoming.updatedAtEpochSeconds < existing.updatedAtEpochSeconds) {
    return existing;
  }

  if (incoming.messages.length === 0 && existing.messages.length > 0) {
    return {
      ...incoming,
      messages: existing.messages,
      hasCoachMessage: incoming.hasCoachMessage ?? existing.hasCoachMessage,
    };
  }

  return incoming;
}

type CoachSessionCacheValue = {
  revision: number;
  getSummary: (workoutId: string) => WorkoutSummary | undefined;
  upsertFullSummary: (summary: WorkoutSummary) => void;
  hydrateMetadataSummaries: (requestedWorkoutIds: string[], summaries: WorkoutSummary[]) => void;
  clearSummaries: (workoutIds: string[]) => void;
};

const CoachSessionCacheContext = createContext<CoachSessionCacheValue | null>(null);

export function CoachSessionCacheProvider({ children }: { children: React.ReactNode }) {
  const summariesRef = useRef<Map<string, WorkoutSummary>>(new Map());
  const [revision, setRevision] = useState(0);

  const getSummary = useCallback((workoutId: string) => summariesRef.current.get(workoutId), []);

  const upsertFullSummary = useCallback((summary: WorkoutSummary) => {
    summariesRef.current.set(summary.workoutId, summary);
    setRevision((current) => current + 1);
  }, []);

  const hydrateMetadataSummaries = useCallback((requestedWorkoutIds: string[], summariesForRequest: WorkoutSummary[]) => {
    const next = new Map(summariesRef.current);

    for (const summary of summariesForRequest) {
      next.set(
        summary.workoutId,
        mergeMetadataIntoSummary(summariesRef.current.get(summary.workoutId), summary),
      );
    }

    summariesRef.current = next;
    setRevision((current) => current + 1);
  }, []);

  const clearSummaries = useCallback((workoutIds: string[]) => {
    const next = new Map(summariesRef.current);

    for (const workoutId of workoutIds) {
      next.delete(workoutId);
    }

    summariesRef.current = next;
    setRevision((current) => current + 1);
  }, []);

  const value = useMemo(
    () => ({
      revision,
      getSummary,
      upsertFullSummary,
      hydrateMetadataSummaries,
      clearSummaries,
    }),
    [clearSummaries, getSummary, hydrateMetadataSummaries, revision, upsertFullSummary],
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
