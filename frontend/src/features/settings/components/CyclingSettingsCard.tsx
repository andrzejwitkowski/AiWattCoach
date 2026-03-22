import { useState, type ChangeEvent, type ComponentType } from 'react';
import { ChevronRight, Heart, RefreshCw, Zap } from 'lucide-react';

import type { UserSettingsResponse } from '../types';
import { updateCycling } from '../api/settings';

type CyclingSettingsCardProps = {
  settings: UserSettingsResponse;
  apiBaseUrl: string;
  onSave: () => void;
};

function computeProfileAccuracy(s: UserSettingsResponse['cycling']): number {
  const fields = [s.fullName, s.age, s.heightCm, s.weightKg, s.ftpWatts, s.hrMaxBpm, s.vo2Max];
  const filled = fields.filter((v) => v != null).length;
  return Math.round((filled / fields.length) * 100);
}

function formatLastZoneUpdate(epochSeconds: number | null): string {
  if (!epochSeconds) return 'Never';
  const diff = Math.floor(Date.now() / 1000) - epochSeconds;
  const days = Math.floor(diff / (24 * 60 * 60));
  if (days === 0) return 'Today';
  if (days === 1) return 'Yesterday';
  return `${days} days ago`;
}

export function CyclingSettingsCard({ settings, apiBaseUrl, onSave }: CyclingSettingsCardProps) {
  const cycling = settings.cycling;
  const [form, setForm] = useState<Record<string, string>>({
    fullName: cycling.fullName ?? '',
    age: cycling.age?.toString() ?? '',
    heightCm: cycling.heightCm?.toString() ?? '',
    weightKg: cycling.weightKg?.toString() ?? '',
    ftpWatts: cycling.ftpWatts?.toString() ?? '',
    hrMaxBpm: cycling.hrMaxBpm?.toString() ?? '',
    vo2Max: cycling.vo2Max?.toString() ?? '',
  });
  const [isSaving, setIsSaving] = useState(false);
  const [saveError, setSaveError] = useState<string | null>(null);

  const accuracy = computeProfileAccuracy(cycling);

  const handleChange = (field: string) => (e: ChangeEvent<HTMLInputElement>) => {
    setForm((prev) => ({ ...prev, [field]: e.target.value }));
    setSaveError(null);
  };

  const handleSave = async () => {
    setIsSaving(true);
    try {
      const req: Record<string, unknown> = {};
      if (form.fullName) req.fullName = form.fullName;
      if (form.age) {
        const age = parseInt(form.age, 10);
        if (!Number.isNaN(age)) req.age = age;
      }
      if (form.heightCm) {
        const h = parseInt(form.heightCm, 10);
        if (!Number.isNaN(h)) req.heightCm = h;
      }
      if (form.weightKg) {
        const w = parseFloat(form.weightKg);
        if (!Number.isNaN(w)) req.weightKg = w;
      }
      if (form.ftpWatts) {
        const ftp = parseInt(form.ftpWatts, 10);
        if (!Number.isNaN(ftp)) req.ftpWatts = ftp;
      }
      if (form.hrMaxBpm) {
        const hr = parseInt(form.hrMaxBpm, 10);
        if (!Number.isNaN(hr)) req.hrMaxBpm = hr;
      }
      if (form.vo2Max) {
        const vo2 = parseFloat(form.vo2Max);
        if (!Number.isNaN(vo2)) req.vo2Max = vo2;
      }
      await updateCycling(apiBaseUrl, req);
      onSave();
    } catch (err) {
      setSaveError(err instanceof Error ? err.message : 'Failed to save cycling settings');
    } finally {
      setIsSaving(false);
    }
  };

  return (
    <div className="rounded-2xl border border-white/10 bg-white/5 p-6 backdrop-blur">
      <div className="flex items-start justify-between gap-4 flex-wrap">
        <div>
          <h2 className="text-2xl font-bold text-white">Cycling Settings</h2>
          <p className="text-[10px] uppercase tracking-[0.2em] text-slate-500 mt-0.5">
            Ustawienia Kolarskie
          </p>
        </div>
        <p className="text-sm text-slate-300 max-w-md leading-relaxed">
          Physiological metrics used for power zone calculation, training load estimation, and AI coaching
          insights. Accurate data improves analysis quality.
        </p>
      </div>

      <div className="mt-6 grid gap-6 lg:grid-cols-3">
        <div className="lg:col-span-2 space-y-4">
          <div className="grid gap-4 sm:grid-cols-2">
            <FormField
              label="Full Name"
              sublabel="Imie i Nazwisko"
              value={form.fullName}
              onChange={handleChange('fullName')}
              placeholder="Alex Rivier"
            />
            <FormField
              label="Age"
              sublabel="Wiek"
              type="number"
              value={form.age}
              onChange={handleChange('age')}
              placeholder="28"
            />
            <FormField
              label="Height"
              sublabel="Wzrost"
              type="number"
              suffix="CM"
              value={form.heightCm}
              onChange={handleChange('heightCm')}
              placeholder="182"
            />
            <FormField
              label="Weight"
              sublabel="Waga"
              type="number"
              suffix="KG"
              value={form.weightKg}
              onChange={handleChange('weightKg')}
              placeholder="74"
            />
          </div>
        </div>

        <div className="rounded-xl border border-white/10 bg-slate-900/40 p-4">
          <p className="text-[10px] uppercase tracking-widest text-slate-500">Profile Accuracy</p>
          <div className="flex items-end gap-2 mt-3">
            <span className="text-4xl font-bold text-cyan-400">{accuracy}%</span>
            <span className="text-sm text-slate-400 mb-1">complete</span>
          </div>
          <div className="mt-3 h-1.5 bg-slate-700 rounded-full overflow-hidden">
            <div
              className="h-full bg-cyan-400 rounded-full transition-all"
              style={{ width: `${accuracy}%` }}
            />
          </div>
          <div className="mt-4 flex items-center gap-2 text-xs text-slate-400">
            <RefreshCw size={12} />
            Last Zone Update: {formatLastZoneUpdate(cycling.lastZoneUpdateEpochSeconds)}
          </div>
        </div>
      </div>

      <div className="mt-6 grid gap-4 sm:grid-cols-2">
        <MetricCard
          icon={Zap}
          iconBg="bg-yellow-400/20"
          iconColor="text-yellow-400"
          label="Functional Threshold Power"
          value={cycling.ftpWatts ? `${cycling.ftpWatts} Watts` : '—'}
          editable
          inputValue={form.ftpWatts}
          onChange={handleChange('ftpWatts')}
          placeholder="280"
        />
        <MetricCard
          icon={Heart}
          iconBg="bg-red-400/20"
          iconColor="text-red-400"
          label="HR Max"
          value={cycling.hrMaxBpm ? `${cycling.hrMaxBpm} BPM` : '—'}
          editable
          inputValue={form.hrMaxBpm}
          onChange={handleChange('hrMaxBpm')}
          placeholder="192"
        />
      </div>

      <div className="mt-6">
        <FormField
          label="VO2 Max"
          sublabel="Maximal Oxygen Uptake"
          type="number"
          value={form.vo2Max}
          onChange={handleChange('vo2Max')}
          placeholder="62.0"
        />
      </div>

      {saveError && (
        <div className="mt-4 rounded-xl border border-red-500/30 bg-red-500/10 px-4 py-3 text-sm text-red-300">
          {saveError}
        </div>
      )}

      <button
        className="mt-6 w-full flex items-center justify-center gap-2 bg-slate-800 border border-white/10 text-slate-300 font-semibold rounded-xl py-3 text-sm uppercase tracking-wider hover:bg-slate-700 transition disabled:opacity-60 disabled:cursor-not-allowed"
        onClick={() => { void handleSave(); }}
        disabled={isSaving}
        type="button"
      >
        {isSaving ? 'Synchronizing...' : <><RefreshCw size={15} />Synchronize Bio-Metrics</>}
      </button>
    </div>
  );
}

function FormField({
  label,
  sublabel,
  type = 'text',
  value,
  onChange,
  placeholder,
  suffix,
}: {
  label: string;
  sublabel: string;
  type?: string;
  value: string;
  onChange: (e: ChangeEvent<HTMLInputElement>) => void;
  placeholder?: string;
  suffix?: string;
}) {
  return (
    <div>
      <label className="block text-xs uppercase tracking-widest text-slate-400 mb-2">
        {label} <span className="text-slate-600 normal-case tracking-normal">/ {sublabel}</span>
      </label>
      <div className="relative">
        <input
          className="w-full bg-slate-900/60 border border-white/10 rounded-xl px-4 py-3 text-slate-200 text-sm placeholder:text-slate-600 focus:outline-none focus:border-cyan-400/50 transition pr-16"
          type={type}
          placeholder={placeholder}
          value={value}
          onChange={onChange}
        />
        {suffix && (
          <span className="absolute right-3 top-1/2 -translate-y-1/2 text-xs text-slate-500 font-medium">
            {suffix}
          </span>
        )}
      </div>
    </div>
  );
}

function MetricCard({
  icon: Icon,
  iconBg,
  iconColor,
  label,
  value,
  editable,
  inputValue,
  onChange,
  placeholder,
}: {
  icon: ComponentType<{ size?: number; className?: string }>;
  iconBg: string;
  iconColor: string;
  label: string;
  value: string;
  editable?: boolean;
  inputValue?: string;
  onChange?: (e: ChangeEvent<HTMLInputElement>) => void;
  placeholder?: string;
}) {
  return (
    <div className="rounded-xl border border-white/10 bg-slate-900/40 p-4">
      <div className="flex items-center gap-3">
        <div className={`w-9 h-9 rounded-lg ${iconBg} flex items-center justify-center`}>
          <Icon size={18} className={iconColor} />
        </div>
        <div className="flex-1">
          <p className="text-[10px] uppercase tracking-widest text-slate-500">{label}</p>
          {editable ? (
            <input
              className="mt-1 bg-transparent text-xl font-bold text-white placeholder:text-slate-600 focus:outline-none w-full"
              type="number"
              placeholder={placeholder}
              value={inputValue}
              onChange={onChange}
            />
          ) : (
            <p className="mt-1 text-xl font-bold text-white">{value}</p>
          )}
        </div>
        <ChevronRight size={16} className="text-slate-500" />
      </div>
    </div>
  );
}
