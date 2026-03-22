import { Info, Settings2 } from 'lucide-react';

import type { AnalysisOptionsSettings, UpdateOptionsRequest } from '../types';

type OptionsCardProps = {
  settings: AnalysisOptionsSettings;
  onToggle: (data: UpdateOptionsRequest) => Promise<void>;
};

export function OptionsCard({ settings, onToggle }: OptionsCardProps) {
  return (
    <div className="rounded-2xl border border-white/10 bg-white/5 p-6 backdrop-blur">
      <div className="flex items-center gap-3">
        <Settings2 size={20} className="text-cyan-400" />
        <h2 className="text-2xl font-bold text-white">Options</h2>
      </div>

      <div className="mt-6 border-t border-white/10 pt-5">
        <div className="flex items-start justify-between gap-4">
          <div>
            <h3 className="text-lg font-bold text-white">Analyze without heart rate</h3>
            <p className="text-[10px] uppercase tracking-widest text-slate-500 mt-0.5">
              Analizuj bez tetna
            </p>
          </div>
          <button
            className="relative w-12 h-6 rounded-full transition-colors focus:outline-none focus:ring-2 focus:ring-cyan-400/50 shrink-0"
            style={{ backgroundColor: settings.analyzeWithoutHeartRate ? '#22d3ee' : '#475569' }}
            onClick={() => {
              void onToggle({ analyzeWithoutHeartRate: !settings.analyzeWithoutHeartRate });
            }}
            type="button"
            role="switch"
            aria-checked={settings.analyzeWithoutHeartRate}
          >
            <span
              className={`absolute top-1 left-1 w-4 h-4 bg-white rounded-full transition-transform ${
                settings.analyzeWithoutHeartRate ? 'translate-x-6' : 'translate-x-0'
              }`}
            />
          </button>
        </div>

        <div className="mt-4 border-l-4 border-cyan-400 bg-slate-800/50 rounded-r-xl p-4 flex gap-3">
          <Info size={16} className="text-cyan-400 shrink-0 mt-0.5" />
          <p className="text-sm text-slate-300 leading-relaxed">
            Enabling this will use Power (Watts) as the sole metric for AI analysis when heart rate data
            is unavailable or unreliable. FTP and stress scores will still be calculated.
          </p>
        </div>

        <div className="mt-4 flex items-center gap-2">
          <span className="w-2 h-2 rounded-full bg-emerald-400" />
          <span className="text-sm text-emerald-400 font-medium">All engines nominal</span>
        </div>
      </div>
    </div>
  );
}
