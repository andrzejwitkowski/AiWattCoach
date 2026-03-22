import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { ApiKeyInput } from './ApiKeyInput';
import type { UserSettingsResponse } from '../types';
import { updateIntervals } from '../api/settings';

type IntervalsCardProps = {
  settings: UserSettingsResponse;
  apiBaseUrl: string;
  onSave: () => void;
};

export function IntervalsCard({ settings, apiBaseUrl, onSave }: IntervalsCardProps) {
  const { t } = useTranslation();
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
          <h3 className="text-lg font-semibold text-white">{t('intervals.title')}</h3>
          <p className="text-xs text-slate-400">{t('intervals.subtitle')}</p>
        </div>
      </div>

      <p className="mb-5 text-sm leading-relaxed text-slate-400">
        {t('intervals.description')}
      </p>

      <div className="space-y-4">
        <ApiKeyInput
          id="intervals-api-key"
          label={t('intervals.apiKey')}
          placeholder="Enter your Intervals API key"
          isConfigured={intervals.apiKeySet}
          configuredLabel={t('intervals.configured')}
          value={apiKey}
          onChange={setApiKey}
          accentColor="emerald"
        />

        <div>
          <label htmlFor="athlete-id" className="mb-1.5 block text-xs font-medium uppercase tracking-wider text-slate-400">
            {t('intervals.athleteId')}
          </label>
          <input
            id="athlete-id"
            type="text"
            className="w-full rounded-xl border border-white/10 bg-white/5 px-4 py-2.5 text-sm text-white placeholder-slate-500 focus:border-emerald-500/50 focus:outline-none focus:ring-1 focus:ring-emerald-500/30"
            placeholder={intervals.athleteId ?? 'i12345678'}
            value={athleteId}
            onChange={(e) => setAthleteId(e.target.value)}
          />
        </div>

        <div className="flex items-center justify-between rounded-xl border border-emerald-500/20 bg-emerald-500/10 px-4 py-3">
          <div className="flex items-center gap-2">
            <div className={`h-2 w-2 rounded-full ${intervals.connected ? 'bg-emerald-400' : 'bg-slate-600'}`} />
            <span className="text-sm text-emerald-300">
              {intervals.connected ? t('intervals.connectedStatus') : t('intervals.notConnectedStatus')}
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
          {isSaving ? t('intervals.connecting') : saved ? t('intervals.connected') : t('intervals.connect')}
        </button>
      </div>
    </div>
  );
}
