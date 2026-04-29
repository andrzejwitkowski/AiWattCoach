import {useEffect, useState} from 'react';
import {useTranslation} from 'react-i18next';

import {AuthenticationError, HttpError} from '../../../lib/httpClient';
import {loadCompletedWorkoutSummary} from '../../intervals/api/intervals';
import type {CompletedWorkoutSummary} from '../../intervals/types';

type UseCompletedWorkoutSummaryOptions = {
  activityId: string | null;
  apiBaseUrl: string;
};

type UseCompletedWorkoutSummaryResult = {
  isLoading: boolean;
  summary: CompletedWorkoutSummary | null;
  summaryError: string | null;
};

export function useCompletedWorkoutSummary({activityId, apiBaseUrl}: UseCompletedWorkoutSummaryOptions): UseCompletedWorkoutSummaryResult {
  const {t} = useTranslation();
  const [summary, setSummary] = useState<CompletedWorkoutSummary | null>(null);
  const [isLoading, setIsLoading] = useState(false);
  const [summaryError, setSummaryError] = useState<string | null>(null);

  useEffect(() => {
    setSummary(null);
    setSummaryError(null);

    if (!activityId) {
      setIsLoading(false);
      return;
    }

    let cancelled = false;
    setIsLoading(true);

    void (async () => {
      try {
        const nextSummary = await loadCompletedWorkoutSummary(apiBaseUrl, activityId);
        if (!cancelled) {
          setSummary(nextSummary);
        }
      } catch (error: unknown) {
        if (cancelled) {
          return;
        }

        if (error instanceof AuthenticationError) {
          window.location.href = '/';
          return;
        }

        if (error instanceof HttpError) {
          if (error.status !== 404) {
            setSummaryError(t('calendar.workoutSummaryUnavailable'));
          }
          return;
        }

        setSummaryError(error instanceof Error ? error.message : t('calendar.workoutSummaryUnavailable'));
      } finally {
        if (!cancelled) {
          setIsLoading(false);
        }
      }
    })();

    return () => {
      cancelled = true;
    };
  }, [activityId, apiBaseUrl, t]);

  return {
    isLoading,
    summary,
    summaryError,
  };
}
