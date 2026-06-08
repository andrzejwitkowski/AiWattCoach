import { useState } from 'react';
import { Eye, EyeOff, RefreshCw } from 'lucide-react';

import { useIntervalsCard } from '../hooks/useIntervalsCard';
import type { UserSettingsResponse } from '../types';
import { SettingsStatusBanner } from './SettingsStatusBanner';

type IntervalsCardProps = {
  settings: UserSettingsResponse;
  apiBaseUrl: string;
  onSave: () => void;
};

export function IntervalsCard({ settings, apiBaseUrl, onSave }: IntervalsCardProps) {
  const {
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
  } = useIntervalsCard({ settings, apiBaseUrl, onSave });
  const [showKey, setShowKey] = useState(false);

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
              onChange={(event) => updateDraft('apiKey', event.target.value)}
            />
            <button
              className="absolute right-3 top-1/2 -translate-y-1/2 text-slate-400 hover:text-slate-200 transition"
              onClick={() => setShowKey((value) => !value)}
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
            onChange={(event) => updateDraft('athleteId', event.target.value)}
          />
          {intervals.athleteId && (
            <p className="mt-1.5 text-xs text-slate-400">Current: {intervals.athleteId}</p>
          )}
        </div>
      </div>

      {status ? <SettingsStatusBanner status={status} /> : null}

      <div className="mt-6 flex gap-3">
        <button
          className="flex-1 flex items-center justify-center gap-2 rounded-xl border border-cyan-400/30 bg-transparent py-3 text-sm font-semibold text-cyan-300 transition hover:bg-cyan-400/10 disabled:cursor-not-allowed disabled:opacity-60"
          onClick={() => {
            void handleTest();
          }}
          disabled={isSaving || isTesting || !canTest}
          type="button"
        >
          <RefreshCw size={15} className={isTesting ? 'animate-spin' : undefined} />
          {isTesting ? 'Testing...' : 'Test Connection'}
        </button>
        <button
          className="flex-1 flex items-center justify-center gap-2 rounded-xl bg-cyan-400 py-3 text-sm font-semibold text-slate-950 transition hover:bg-cyan-300 disabled:cursor-not-allowed disabled:opacity-60"
          onClick={() => {
            void handleSave();
          }}
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
