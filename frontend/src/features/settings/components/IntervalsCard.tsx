import { useEffect, useMemo, useRef, useState } from 'react';
import { AlertCircle, CheckCircle2, Eye, EyeOff, RefreshCw } from 'lucide-react';
import type { TestIntervalsConnectionResponse, UserSettingsResponse } from '../types';
import { testIntervalsConnection, updateIntervals } from '../api/settings';

type IntervalsCardProps = {
  settings: UserSettingsResponse;
  apiBaseUrl: string;
  onSave: () => void;
};

export function IntervalsCard({ settings, apiBaseUrl, onSave }: IntervalsCardProps) {
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
  const [showKey, setShowKey] = useState(false);
  const [isSaving, setIsSaving] = useState(false);
  const [isTesting, setIsTesting] = useState(false);
  const [status, setStatus] = useState<{
    tone: 'neutral' | 'success' | 'error';
    label: string;
    message: string;
  } | null>(null);
  const previousPersistedRef = useRef({
    apiKey: persistedApiKey,
    athleteId: persistedAthleteId,
  });
  const testRunIdRef = useRef(0);

  useEffect(() => {
    const previousPersisted = previousPersistedRef.current;

    setDraft((current) => ({
      apiKey: current.apiKey === previousPersisted.apiKey ? persistedApiKey : current.apiKey,
      athleteId:
        current.athleteId === previousPersisted.athleteId ? persistedAthleteId : current.athleteId,
    }));
    setCleanDraft((current) => ({
      apiKey: current.apiKey === previousPersisted.apiKey ? persistedApiKey : current.apiKey,
      athleteId:
        current.athleteId === previousPersisted.athleteId ? persistedAthleteId : current.athleteId,
    }));
    previousPersistedRef.current = {
      apiKey: persistedApiKey,
      athleteId: persistedAthleteId,
    };
  }, [persistedApiKey, persistedAthleteId]);

  const hasSavedCompleteCredentials = intervals.apiKeySet && Boolean(intervals.athleteId);
  const hasDirtyDraft =
    draft.apiKey.trim() !== cleanDraft.apiKey.trim() ||
    draft.athleteId.trim() !== cleanDraft.athleteId.trim();
  const canReconnectSavedCredentials = hasSavedCompleteCredentials && !intervals.connected && !hasDirtyDraft;

  const saveRequest = useMemo(() => {
    const trimmedApiKey = draft.apiKey.trim();
    const trimmedAthleteId = draft.athleteId.trim();
    const cleanApiKey = cleanDraft.apiKey.trim();
    const cleanAthleteId = cleanDraft.athleteId.trim();
    const request: Record<string, string | null> = {};

    if (trimmedApiKey !== cleanApiKey) {
      request.apiKey = trimmedApiKey ? trimmedApiKey : null;
    }
    if (trimmedAthleteId !== cleanAthleteId) {
      request.athleteId = trimmedAthleteId ? trimmedAthleteId : null;
    }

    return request;
  }, [cleanDraft.apiKey, cleanDraft.athleteId, draft.apiKey, draft.athleteId]);

  const visibleTestRequest = useMemo(() => {
    const trimmedApiKey = draft.apiKey.trim();
    const trimmedAthleteId = draft.athleteId.trim();
    const cleanApiKey = cleanDraft.apiKey.trim();
    const cleanAthleteId = cleanDraft.athleteId.trim();
    const request: Record<string, string> = {};

    if (trimmedApiKey && trimmedApiKey !== cleanApiKey) {
      request.apiKey = trimmedApiKey;
    }
    if (trimmedAthleteId && trimmedAthleteId !== cleanAthleteId) {
      request.athleteId = trimmedAthleteId;
    }

    return request;
  }, [cleanDraft.apiKey, cleanDraft.athleteId, draft.apiKey, draft.athleteId]);

  const canSave = Object.keys(saveRequest).length > 0 || canReconnectSavedCredentials;
  const canTest = Object.keys(visibleTestRequest).length > 0 || hasSavedCompleteCredentials;

  const clearTestStatusIfNeeded = () => {
    testRunIdRef.current += 1;
    setIsTesting(false);
    setStatus((current) => {
      if (!current) return current;
      return null;
    });
  };

  const setStatusFromTest = (result: TestIntervalsConnectionResponse) => {
    setStatus({
      tone: result.connected ? 'success' : 'error',
      label: result.connected ? 'OK' : 'FAILED',
      message: result.message,
    });
  };

  const handleSave = async () => {
    if (!canSave) return;
    setIsSaving(true);
    setStatus({
      tone: 'neutral',
      label: 'Saving',
      message: 'Saving current Intervals.icu credentials...',
    });
    try {
      await updateIntervals(apiBaseUrl, canReconnectSavedCredentials ? {} : saveRequest);
      setCleanDraft(draft);
      setStatus({
        tone: 'success',
        label: 'Saved',
        message: 'Credentials saved.',
      });
      onSave();
    } catch (err) {
      setStatus({
        tone: 'error',
        label: 'Save failed',
        message: err instanceof Error ? err.message : 'Failed to save Intervals.icu credentials',
      });
    } finally {
      setIsSaving(false);
    }
  };

  const handleTest = async () => {
    if (!canTest) return;
    const testRunId = testRunIdRef.current + 1;
    testRunIdRef.current = testRunId;
    const submittedTestRequest = visibleTestRequest;
    setIsTesting(true);
    setStatus({
      tone: 'neutral',
      label: 'Testing',
      message: 'Testing current Intervals.icu values...',
    });
    try {
      const result = await testIntervalsConnection(apiBaseUrl, submittedTestRequest);
      if (testRunId !== testRunIdRef.current) {
        if (result.persistedStatusUpdated) {
          setCleanDraft((current) => ({
            apiKey: submittedTestRequest.apiKey ?? current.apiKey,
            athleteId: submittedTestRequest.athleteId ?? current.athleteId,
          }));
          onSave();
        }
        return;
      }
      setStatusFromTest(result);
      if (result.persistedStatusUpdated) {
        setCleanDraft(draft);
        onSave();
      }
    } catch (err) {
      if (testRunId !== testRunIdRef.current) {
        return;
      }
      setStatus({
        tone: 'error',
        label: 'FAILED',
        message: err instanceof Error ? err.message : 'Failed to test Intervals.icu connection',
      });
    } finally {
      if (testRunId === testRunIdRef.current) {
        setIsTesting(false);
      }
    }
  };

  const statusClasses =
    status?.tone === 'success'
      ? 'border-emerald-400/30 bg-emerald-500/10 text-emerald-200'
      : status?.tone === 'error'
        ? 'border-red-500/30 bg-red-500/10 text-red-200'
        : 'border-cyan-400/20 bg-cyan-400/10 text-cyan-100';

  const StatusIcon = status?.tone === 'success' ? CheckCircle2 : status?.tone === 'error' ? AlertCircle : RefreshCw;

  return (
    <div className="rounded-2xl border border-white/10 bg-white/5 p-6 backdrop-blur">
      <div className="flex items-start gap-4">
        <div className="w-10 h-10 rounded-xl bg-cyan-400/20 flex items-center justify-center shrink-0">
          <RefreshCw size={20} className="text-cyan-400" />
        </div>
        <div className="flex-1">
          <h2 className="text-xl font-bold text-white">Intervals.icu</h2>
          <p className="text-[10px] uppercase tracking-[0.2em] text-slate-500 mt-0.5">
            External Ecosystem
          </p>
        </div>
        {intervals.connected && (
          <span className="text-[10px] font-semibold bg-emerald-400/20 text-emerald-400 rounded-full px-2 py-0.5 uppercase tracking-wider">
            Connected
          </span>
        )}
      </div>

      <p className="mt-4 text-sm text-slate-300 leading-relaxed">
        Connect your Intervals.icu account to sync training data, load zones, and enable AI-powered analysis.
      </p>

      <div className="mt-6 space-y-4">
        <div>
          <label htmlFor="intervals-api-key" className="block text-xs uppercase tracking-widest text-slate-400 mb-2">
            API Key
          </label>
          <div className="relative">
            <input
              id="intervals-api-key"
              className="w-full bg-slate-900/60 border border-white/10 rounded-xl px-4 py-3 pr-10 text-slate-200 text-sm placeholder:text-slate-600 focus:outline-none focus:border-cyan-400/50 transition"
              type={showKey ? 'text' : 'password'}
              placeholder={intervals.apiKeySet ? 'Already configured' : 'Enter API key'}
              value={draft.apiKey}
              onChange={(e) => {
                clearTestStatusIfNeeded();
                const value = e.target.value;
                setDraft((current) => ({ ...current, apiKey: value }));
              }}
            />
            <button
              className="absolute right-3 top-1/2 -translate-y-1/2 text-slate-400 hover:text-slate-200 transition"
              onClick={() => setShowKey((v) => !v)}
              type="button"
              aria-label={showKey ? 'Hide key' : 'Show key'}
            >
              {showKey ? <EyeOff size={16} /> : <Eye size={16} />}
            </button>
          </div>
          {intervals.apiKeySet && (
            <p className="mt-1.5 text-xs text-emerald-400">API key is configured</p>
          )}
        </div>

        <div>
          <label htmlFor="intervals-athlete-id" className="block text-xs uppercase tracking-widest text-slate-400 mb-2">
            Athlete ID
          </label>
            <input
              id="intervals-athlete-id"
              className="w-full bg-slate-900/60 border border-white/10 rounded-xl px-4 py-3 text-slate-200 text-sm placeholder:text-slate-600 focus:outline-none focus:border-cyan-400/50 transition"
              type="text"
              placeholder={intervals.athleteId ?? 'i123456'}
              value={draft.athleteId}
              onChange={(e) => {
                clearTestStatusIfNeeded();
                const value = e.target.value;
                setDraft((current) => ({ ...current, athleteId: value }));
              }}
            />
          {intervals.athleteId && (
            <p className="mt-1.5 text-xs text-slate-400">Current: {intervals.athleteId}</p>
          )}
        </div>
      </div>

      {status && (
        <div className={`mt-4 rounded-xl border px-4 py-3 text-sm ${statusClasses}`}>
          <div className="flex items-start gap-3">
            <StatusIcon size={16} className={status?.tone === 'neutral' ? 'animate-spin shrink-0 mt-0.5' : 'shrink-0 mt-0.5'} />
            <div>
              <p className="font-semibold uppercase tracking-wider text-[11px]">{status.label}</p>
              <p className="mt-1">{status.message}</p>
            </div>
          </div>
        </div>
      )}

      <div className="mt-6 flex gap-3">
        <button
          className="flex-1 flex items-center justify-center gap-2 rounded-xl border border-cyan-400/30 bg-transparent py-3 text-sm font-semibold text-cyan-300 transition hover:bg-cyan-400/10 disabled:cursor-not-allowed disabled:opacity-60"
          onClick={() => { void handleTest(); }}
          disabled={isSaving || isTesting || !canTest}
          type="button"
        >
          <RefreshCw size={15} className={isTesting ? 'animate-spin' : undefined} />
          {isTesting ? 'Testing...' : 'Test Connection'}
        </button>
        <button
          className="flex-1 flex items-center justify-center gap-2 rounded-xl bg-cyan-400 py-3 text-sm font-semibold text-slate-950 transition hover:bg-cyan-300 disabled:cursor-not-allowed disabled:opacity-60"
          onClick={() => { void handleSave(); }}
          disabled={isSaving || isTesting || !canSave}
          type="button"
        >
          <RefreshCw size={15} className={isSaving ? 'animate-spin' : undefined} />
          {isSaving ? 'Saving...' : 'Connect Intervals'}
        </button>
      </div>
    </div>
  );
}
