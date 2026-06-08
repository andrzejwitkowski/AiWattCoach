import { useEffect, useMemo, useRef, useState } from 'react';

import { updateIntervals } from '../api/settings';
import { buildIntervalsSaveRequest, buildIntervalsTestRequest } from '../intervalsDraft';
import type { SettingsStatus } from '../components/SettingsStatusBanner';
import { connectionErrorStatus } from '../components/settingsCardFlow';
import { runIntervalsConnectionTest } from '../components/intervalsCardFlow';
import type { UserSettingsResponse } from '../types';

type IntervalsDraft = {
  apiKey: string;
  athleteId: string;
};

type UseIntervalsCardOptions = {
  settings: UserSettingsResponse;
  apiBaseUrl: string;
  onSave: () => void;
};

function mergeIntervalsDraft(
  current: IntervalsDraft,
  previousPersisted: IntervalsDraft,
  persisted: IntervalsDraft,
): IntervalsDraft {
  return {
    apiKey: current.apiKey === previousPersisted.apiKey ? persisted.apiKey : current.apiKey,
    athleteId: current.athleteId === previousPersisted.athleteId ? persisted.athleteId : current.athleteId,
  };
}

export function useIntervalsCard({ settings, apiBaseUrl, onSave }: UseIntervalsCardOptions) {
  const intervals = settings.intervals;
  const persistedApiKey = intervals.apiKey ?? '';
  const persistedAthleteId = intervals.athleteId ?? '';
  const [draft, setDraft] = useState({
    apiKey: persistedApiKey,
    athleteId: persistedAthleteId,
  });
  const [cleanDraft, setCleanDraft] = useState({
    apiKey: persistedApiKey,
    athleteId: persistedAthleteId,
  });
  const [isSaving, setIsSaving] = useState(false);
  const [isTesting, setIsTesting] = useState(false);
  const [status, setStatus] = useState<SettingsStatus | null>(null);
  const previousPersistedRef = useRef({
    apiKey: persistedApiKey,
    athleteId: persistedAthleteId,
  });
  const testRunIdRef = useRef(0);

  useEffect(() => {
    const previousPersisted = previousPersistedRef.current;
    const persisted = { apiKey: persistedApiKey, athleteId: persistedAthleteId };
    setDraft((current) => mergeIntervalsDraft(current, previousPersisted, persisted));
    setCleanDraft((current) => mergeIntervalsDraft(current, previousPersisted, persisted));
    previousPersistedRef.current = persisted;
  }, [persistedApiKey, persistedAthleteId]);

  const hasSavedCompleteCredentials = intervals.apiKeySet && Boolean(intervals.athleteId);
  const hasDirtyDraft =
    draft.apiKey.trim() !== cleanDraft.apiKey.trim() ||
    draft.athleteId.trim() !== cleanDraft.athleteId.trim();
  const canReconnectSavedCredentials = hasSavedCompleteCredentials && !intervals.connected && !hasDirtyDraft;
  const saveRequest = useMemo(() => buildIntervalsSaveRequest(draft, cleanDraft), [cleanDraft, draft]);
  const visibleTestRequest = useMemo(() => buildIntervalsTestRequest(draft, cleanDraft), [cleanDraft, draft]);
  const canSave = Object.keys(saveRequest).length > 0 || canReconnectSavedCredentials;
  const canTest = Object.keys(visibleTestRequest).length > 0 || hasSavedCompleteCredentials;

  const clearTestStatusIfNeeded = () => {
    testRunIdRef.current += 1;
    setIsTesting(false);
    setStatus(null);
  };

  const updateDraft = (field: keyof IntervalsDraft, value: string) => {
    clearTestStatusIfNeeded();
    setDraft((current) => ({ ...current, [field]: value }));
  };

  const handleTest = async () => {
    if (!canTest) {
      return;
    }

    const testRunId = testRunIdRef.current + 1;
    testRunIdRef.current = testRunId;
    const submittedTestRequest = visibleTestRequest;
    setIsTesting(true);
    setStatus({
      tone: 'neutral',
      label: 'Testing',
      message: 'Testing current Intervals.icu values...',
    });

    await runIntervalsConnectionTest({
      apiBaseUrl,
      submittedTestRequest,
      draft,
      testRunId,
      getCurrentTestRunId: () => testRunIdRef.current,
      onStalePersistedUpdate: (request) => {
        if (request.apiKey !== undefined || request.athleteId !== undefined) {
          setCleanDraft((current) => ({
            apiKey: request.apiKey ?? current.apiKey,
            athleteId: request.athleteId ?? current.athleteId,
          }));
        }
      },
      onSave,
      setStatus,
      setCleanDraft,
      setIsTesting,
    });
  };

  const handleSave = async () => {
    if (!canSave) {
      return;
    }

    if (canReconnectSavedCredentials) {
      await handleTest();
      return;
    }

    setIsSaving(true);
    setStatus({
      tone: 'neutral',
      label: 'Saving',
      message: 'Saving current Intervals.icu credentials...',
    });

    try {
      await updateIntervals(apiBaseUrl, saveRequest);
      setCleanDraft(draft);
      setStatus({
        tone: 'success',
        label: 'Saved',
        message: 'Credentials saved.',
      });
      onSave();
    } catch (err) {
      setStatus(connectionErrorStatus(err, 'Failed to save Intervals.icu credentials'));
    } finally {
      setIsSaving(false);
    }
  };

  return {
    intervals,
    draft,
    status,
    isSaving,
    isTesting,
    canSave,
    canTest,
    updateDraft,
    handleSave,
    handleTest,
  };
}
