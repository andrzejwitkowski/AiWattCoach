import { useState } from 'react';
import type { UserSettingsResponse } from '../types';
import { updateIntervals } from '../api/settings';
import type { UpdateIntervalsRequest } from '../types';

type IntervalsCardProps = {
  settings: UserSettingsResponse;
  apiBaseUrl: string;
  onSave: () => void;
};

export function IntervalsCard({ settings, apiBaseUrl, onSave }: IntervalsCardProps) {
  const [apiKey, setApiKey] = useState('');
  const [athleteId, setAthleteId] = useState('');
  const [isSaving, setIsSaving] = useState(false);
  const [saved, setSaved] = useState(false);
  const [saveError, setSaveError] = useState<string | null>(null);

  const intervals = settings.intervals;

  async function handleSave() {
    const trimmedApiKey = apiKey.trim();
    const trimmedAthleteId = athleteId.trim();
    if (!trimmedApiKey && !trimmedAthleteId) return;
    setIsSaving(true);
    setSaved(false);
    setSaveError(null);
    try {
      const req: UpdateIntervalsRequest = {};
      if (trimmedApiKey) req.apiKey = trimmedApiKey;
      if (trimmedAthleteId) req.athleteId = trimmedAthleteId;
      await updateIntervals(apiBaseUrl, req);
      setApiKey('');
      setSaved(true);
      onSave();
    } catch (err) {
      setSaveError(err instanceof Error ? err.message : 'Failed to connect to Intervals.icu');
    } finally {
      setIsSaving(false);
    }
  }

  function handleApiKeyChange(value: string) {
    setApiKey(value);
    setSaved(false);
    setSaveError(null);
  }

  function handleAthleteIdChange(value: string) {
    setAthleteId(value);
    setSaved(false);
    setSaveError(null);
  }

  return (
    <div className="rounded-[1.5rem] border border-white/10 bg-gradient-to-br from-emerald-900/40 to-cyan-900/30 p-6">
      <div className="mb-4 flex items-center gap-3">
        <div className="flex h-10 w-10 items-center justify-center rounded-xl bg-emerald-500/15">
          <svg className="h-5 w-5 text-emerald-400" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={1.5}>
            <path strokeLinecap="round" strokeLinejoin="round" d="M7.5 21L3 16.5m0 0L7.5 12M3 16.5h13.5m0-13.5L21 7.5m0 0L16.5 12M21 7.5H7.5" />
          </svg>
        </div>
        <div>
          <h3 className="text-lg font-semibold text-white">Intervals.icu</h3>
          <p className="text-xs text-slate-400">Sync workout data</p>
        </div>
      </div>

      <p className="mb-5 text-sm leading-relaxed text-slate-400">
        Connect your Intervals.icu account to sync workout data and performance metrics automatically.
      </p>

      <div className="space-y-4">
        <div>
          <label className="mb-1.5 block text-xs font-medium uppercase tracking-wider text-slate-400" htmlFor="intervals-api-key">
            API Key
          </label>
          <div className="relative">
            <input
              id="intervals-api-key"
              type="password"
              className="w-full rounded-xl border border-white/10 bg-white/5 px-4 py-2.5 text-sm text-white placeholder-slate-500 focus:border-emerald-500/50 focus:outline-none focus:ring-1 focus:ring-emerald-500/30"
              placeholder={intervals.apiKeySet ? '••••••••••••••••' : 'Enter your Intervals API key'}
              value={apiKey}
              onChange={(e) => handleApiKeyChange(e.target.value)}
            />
            {intervals.apiKeySet && (
              <span className="absolute right-3 top-1/2 -translate-y-1/2 text-xs text-emerald-400">Configured</span>
            )}
          </div>
        </div>

        <div>
          <label className="mb-1.5 block text-xs font-medium uppercase tracking-wider text-slate-400" htmlFor="athlete-id">
            Athlete ID
          </label>
          <input
            id="athlete-id"
            type="text"
            className="w-full rounded-xl border border-white/10 bg-white/5 px-4 py-2.5 text-sm text-white placeholder-slate-500 focus:border-emerald-500/50 focus:outline-none focus:ring-1 focus:ring-emerald-500/30"
            placeholder={intervals.athleteId ?? 'i12345678'}
            value={athleteId}
            onChange={(e) => handleAthleteIdChange(e.target.value)}
          />
        </div>

        <div className="flex items-center justify-between rounded-xl border border-emerald-500/20 bg-emerald-500/10 px-4 py-3">
          <div className="flex items-center gap-2">
            <div className={`h-2 w-2 rounded-full ${intervals.connected ? 'bg-emerald-400' : 'bg-slate-600'}`} />
            <span className="text-sm text-emerald-300">
              {intervals.connected ? 'Connected' : 'Not connected'}
            </span>
          </div>
        </div>

        {saveError && (
          <div className="rounded-xl border border-red-500/30 bg-red-500/10 px-4 py-3 text-sm text-red-300">
            {saveError}
          </div>
        )}

        <button
          type="button"
          className="flex w-full items-center justify-center gap-2 rounded-xl bg-emerald-500 px-4 py-2.5 text-sm font-semibold text-slate-950 transition hover:bg-emerald-400 disabled:opacity-50"
          disabled={isSaving || (!apiKey.trim() && !athleteId.trim())}
          onClick={() => { void handleSave(); }}
        >
          {isSaving ? 'Connecting...' : saved ? 'Connected!' : 'Connect Intervals'}
        </button>
      </div>
    </div>
  );
}
