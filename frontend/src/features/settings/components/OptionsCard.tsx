import { useState } from 'react';
import type { UserSettingsResponse } from '../types';
import { updateOptions } from '../api/settings';
import type { UpdateOptionsRequest } from '../types';

type OptionsCardProps = {
  settings: UserSettingsResponse;
  apiBaseUrl: string;
  onSave: () => void;
};

function Toggle({ enabled, onChange, id }: { enabled: boolean; onChange: (v: boolean) => void; id: string }) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={enabled}
      id={id}
      className={`relative inline-flex h-6 w-11 flex-shrink-0 cursor-pointer rounded-full border-2 border-transparent transition-colors duration-200 ease-in-out focus:outline-none focus-visible:ring-2 focus-visible:ring-cyan-500 focus-visible:ring-offset-2 focus-visible:ring-offset-slate-900 ${
        enabled ? 'bg-cyan-500' : 'bg-white/10'
      }`}
      onClick={() => onChange(!enabled)}
    >
      <span
        className={`pointer-events-none inline-block h-5 w-5 transform rounded-full bg-white shadow-lg ring-0 transition duration-200 ease-in-out ${
          enabled ? 'translate-x-5' : 'translate-x-0'
        }`}
      />
    </button>
  );
}

export function OptionsCard({ settings, apiBaseUrl, onSave }: OptionsCardProps) {
  const [analyzeWithoutHR, setAnalyzeWithoutHR] = useState(settings.options.analyzeWithoutHeartRate);
  const [saveError, setSaveError] = useState<string | null>(null);

  async function handleToggle(value: boolean) {
    setAnalyzeWithoutHR(value);
    setSaveError(null);
    try {
      const req: UpdateOptionsRequest = { analyzeWithoutHeartRate: value };
      await updateOptions(apiBaseUrl, req);
      onSave();
    } catch (err) {
      setAnalyzeWithoutHR(!value);
      setSaveError(err instanceof Error ? err.message : 'Failed to update option');
      setTimeout(() => setSaveError(null), 4000);
    }
  }

  return (
    <div className="rounded-[1.5rem] border border-white/10 bg-white/5 p-6">
      <div className="mb-4 flex items-center gap-3">
        <div className="flex h-10 w-10 items-center justify-center rounded-xl bg-amber-500/15">
          <svg className="h-5 w-5 text-amber-400" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={1.5}>
            <path strokeLinecap="round" strokeLinejoin="round" d="M10.5 6h9.75M10.5 6a1.5 1.5 0 11-3 0m3 0a1.5 1.5 0 10-3 0M3.75 6H7.5m3 12h9.75m-9.75 0a1.5 1.5 0 01-3 0m3 0a1.5 1.5 0 00-3 0m-3.75 0H7.5m9-6h3.75m-3.75 0a1.5 1.5 0 01-3 0m3 0a1.5 1.5 0 00-3 0m-9.75 0h9.75" />
          </svg>
        </div>
        <div>
          <h3 className="text-lg font-semibold text-white">Options</h3>
          <p className="text-xs text-slate-400">Analysis configuration</p>
        </div>
      </div>

      <div className="space-y-5">
        <div className="flex items-center justify-between">
          <div>
            <label className="text-sm font-medium text-white" htmlFor="analyze-without-hr">
              Analyze without heart rate
            </label>
            <p className="text-xs font-medium uppercase tracking-wider text-amber-300">ANALIZUJ BEZ TĘTNA</p>
          </div>
          <Toggle enabled={analyzeWithoutHR} onChange={handleToggle} id="analyze-without-hr" />
        </div>

        <div className="flex items-start gap-3 rounded-xl border border-amber-500/20 bg-amber-500/10 p-4">
          <svg className="mt-0.5 h-4 w-4 flex-shrink-0 text-amber-400" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={1.5}>
            <path strokeLinecap="round" strokeLinejoin="round" d="M11.25 11.25l.041-.02a.75.75 0 011.063.852l-.708 2.836a.75.75 0 001.063.853l.041-.021M21 12a9 9 0 11-18 0 9 9 0 0118 0zm-9-3.75h.008v.008H12V8.25z" />
          </svg>
          <p className="text-xs leading-relaxed text-slate-300">
            Enabling this will use Power (Watts) as the sole metric for training load calculations when HR data is missing.
          </p>
        </div>

        {saveError && (
          <div className="rounded-xl border border-red-500/30 bg-red-500/10 px-4 py-3 text-sm text-red-300">
            {saveError}
          </div>
        )}

        <div className="flex items-center justify-between rounded-xl border border-white/10 bg-white/5 px-4 py-3">
          <div className="flex items-center gap-2">
            <div className="h-2 w-2 rounded-full bg-emerald-400" />
            <span className="text-sm text-emerald-300">All engines nominal</span>
          </div>
          <span className="text-xs text-slate-500">System Status</span>
        </div>
      </div>
    </div>
  );
}
