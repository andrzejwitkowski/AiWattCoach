import { useState } from 'react';
import { Eye, EyeOff, RefreshCw } from 'lucide-react';
import type { UserSettingsResponse } from '../types';
import { updateIntervals } from '../api/settings';

type IntervalsCardProps = {
  settings: UserSettingsResponse;
  apiBaseUrl: string;
  onSave: () => void;
};

export function IntervalsCard({ settings, apiBaseUrl, onSave }: IntervalsCardProps) {
  const [apiKey, setApiKey] = useState('');
  const [athleteId, setAthleteId] = useState('');
  const [showKey, setShowKey] = useState(false);
  const [isSaving, setIsSaving] = useState(false);
  const [saved, setSaved] = useState(false);
  const [saveError, setSaveError] = useState<string | null>(null);

  const intervals = settings.intervals;

  const handleSave = async () => {
    const trimmedApiKey = apiKey.trim();
    const trimmedAthleteId = athleteId.trim();
    if (!trimmedApiKey && !trimmedAthleteId) return;
    setIsSaving(true);
    setSaved(false);
    setSaveError(null);
    try {
      const req: Record<string, string> = {};
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
  };

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
              value={apiKey}
              onChange={(e) => setApiKey(e.target.value)}
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
            value={athleteId}
            onChange={(e) => setAthleteId(e.target.value)}
          />
          {intervals.athleteId && (
            <p className="mt-1.5 text-xs text-slate-400">Current: {intervals.athleteId}</p>
          )}
        </div>
      </div>

      {saveError && (
        <div className="mt-4 rounded-xl border border-red-500/30 bg-red-500/10 px-4 py-3 text-sm text-red-300">
          {saveError}
        </div>
      )}

      <button
        className="mt-6 w-full flex items-center justify-center gap-2 bg-cyan-400 text-slate-950 font-semibold rounded-xl py-3 text-sm hover:bg-cyan-300 transition disabled:opacity-60 disabled:cursor-not-allowed"
        onClick={() => { void handleSave(); }}
        disabled={isSaving || (!apiKey.trim() && !athleteId.trim())}
        type="button"
      >
        {isSaving ? 'Connecting...' : saved ? 'Connected!' : <><RefreshCw size={15} />Connect Intervals</>}
      </button>
    </div>
  );
}
