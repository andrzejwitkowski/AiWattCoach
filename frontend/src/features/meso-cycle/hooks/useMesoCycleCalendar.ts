import { useCallback, useEffect, useMemo, useRef, useState } from 'react';

import { useApiBaseUrl } from '../../../lib/apiBaseUrl';
import { addDays, parseDateKey, toDateKey } from '../../calendar/utils/dateUtils';
import { isLlmProviderKeyConfigured } from '../../settings/llmProviders';
import type { UserSettingsResponse } from '../../settings/types';
import { generateMesoCyclePlan, loadMesoCycleCalendar, loadMesoCycleStatus } from '../api/mesoCycle';
import type { MesoCycleCalendarDay, MesoCycleStatus } from '../types';

const POLL_INTERVAL_MS = 3000;
const MAX_POLL_ATTEMPTS = 120;

type UseMesoCycleCalendarOptions = {
  settings: UserSettingsResponse | null;
};

type MesoCycleCalendarState = {
  status: MesoCycleStatus | null;
  days: MesoCycleCalendarDay[];
  isLoading: boolean;
  isGenerating: boolean;
  error: string | null;
};

export function canGenerateMesoCycle(settings: UserSettingsResponse | null): boolean {
  if (!settings) {
    return false;
  }

  const ai = settings.aiAgents;
  const provider = ai.mesoCycleProvider ?? ai.selectedProvider;
  const model = ai.mesoCycleModel ?? ai.selectedModel;
  if (!provider || !model?.trim()) {
    return false;
  }

  return isLlmProviderKeyConfigured(provider, ai);
}

export function useMesoCycleCalendar({ settings }: UseMesoCycleCalendarOptions) {
  const apiBaseUrl = useApiBaseUrl();
  const [state, setState] = useState<MesoCycleCalendarState>({
    status: null,
    days: [],
    isLoading: true,
    isGenerating: false,
    error: null,
  });
  const refreshInFlightRef = useRef(false);
  const pollAttemptsRef = useRef(0);

  const canGenerate = useMemo(() => canGenerateMesoCycle(settings), [settings]);

  const windowRange = useMemo(() => {
    if (!state.status?.window) {
      return null;
    }

    return {
      from: state.status.window.mesoStart,
      to: state.status.window.mesoEnd,
    };
  }, [state.status?.window]);

  const refresh = useCallback(async () => {
    if (refreshInFlightRef.current) {
      return;
    }

    refreshInFlightRef.current = true;
    setState((current) => ({ ...current, isLoading: current.days.length === 0, error: null }));

    try {
      const status = await loadMesoCycleStatus(apiBaseUrl);
      let days: MesoCycleCalendarDay[] = [];

      if (status.window) {
        days = await loadMesoCycleCalendar(
          apiBaseUrl,
          status.window.mesoStart,
          status.window.mesoEnd,
        );
      }

      setState({
        status,
        days,
        isLoading: false,
        isGenerating: status.hasPendingGeneration,
        error: null,
      });
    } catch (error) {
      setState((current) => ({
        ...current,
        isLoading: false,
        error: error instanceof Error ? error.message : 'Failed to load meso cycle calendar',
      }));
    } finally {
      refreshInFlightRef.current = false;
    }
  }, [apiBaseUrl]);

  const generate = useCallback(async () => {
    if (!canGenerate) {
      return;
    }

    setState((current) => ({ ...current, error: null }));
    pollAttemptsRef.current = 0;

    try {
      await generateMesoCyclePlan(apiBaseUrl);
      setState((current) => ({ ...current, isGenerating: true }));
      try {
        await refresh();
      } catch (refreshError) {
        setState((current) => ({
          ...current,
          error:
            refreshError instanceof Error
              ? refreshError.message
              : 'Failed to refresh meso cycle calendar',
        }));
      }
    } catch (error) {
      setState((current) => ({
        ...current,
        isGenerating: false,
        error: error instanceof Error ? error.message : 'Failed to start meso cycle generation',
      }));
    }
  }, [apiBaseUrl, canGenerate, refresh]);

  useEffect(() => {
    let cancelled = false;

    void (async () => {
      if (!cancelled) {
        await refresh();
      }
    })();

    return () => {
      cancelled = true;
    };
  }, [refresh]);

  useEffect(() => {
    if (!state.isGenerating) {
      pollAttemptsRef.current = 0;
      return undefined;
    }

    const intervalId = window.setInterval(() => {
      pollAttemptsRef.current += 1;
      if (pollAttemptsRef.current > MAX_POLL_ATTEMPTS) {
        setState((current) => ({
          ...current,
          isGenerating: false,
          error: current.error ?? 'Meso cycle generation is taking longer than expected.',
        }));
        window.clearInterval(intervalId);
        return;
      }

      void refresh();
    }, POLL_INTERVAL_MS);

    return () => {
      window.clearInterval(intervalId);
    };
  }, [refresh, state.isGenerating]);

  const orderedDates = useMemo(() => {
    if (!windowRange) {
      return [];
    }

    const dates: string[] = [];
    let cursor = parseDateKey(windowRange.from);
    const end = parseDateKey(windowRange.to);

    while (cursor <= end) {
      dates.push(toDateKey(cursor));
      cursor = addDays(cursor, 1);
    }

    return dates;
  }, [windowRange]);

  const daysByDate = useMemo(() => {
    return new Map(state.days.map((day) => [day.date, day]));
  }, [state.days]);

  return {
    ...state,
    canGenerate,
    windowRange,
    orderedDates,
    daysByDate,
    refresh,
    generate,
  };
}
