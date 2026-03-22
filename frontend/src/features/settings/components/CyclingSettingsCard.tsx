import { useState } from 'react';
import { ChevronRight, Heart, RefreshCw, Zap } from 'lucide-react';

import type { CyclingSettingsData, UpdateCyclingRequest } from '../types';

type CyclingSettingsCardProps = {
  settings: CyclingSettingsData;
  onSave: (data: UpdateCyclingRequest) => Promise<void>;
  isSaving: boolean;
};

function computeProfileAccuracy(s: CyclingSettingsData): number {
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

export function CyclingSettingsCard({ settings, onSave, isSaving }: CyclingSettingsCardProps) {
  const [form, setForm] = useState<Record<string, string>>({
    fullName: settings.fullName ?? '',
    age: settings.age?.toString() ?? '',
    heightCm: settings.heightCm?.toString() ?? '',
    weightKg: settings.weightKg?.toString() ?? '',
    ftpWatts: settings.ftpWatts?.toString() ?? '',
    hrMaxBpm: settings.hrMaxBpm?.toString() ?? '',
    vo2Max: settings.vo2Max?.toString() ?? '',
  });

  const accuracy = computeProfileAccuracy(settings);

  const handleChange = (field: keyof typeof form) => (e: React.ChangeEvent<HTMLInputElement>) => {
    setForm((prev) => ({ ...prev, [field]: e.target.value }));
  };

  const handleSave = async () => {
    await onSave({
      fullName: form.fullName || null,
      age: form.age ? Number(form.age) : null,
      heightCm: form.heightCm ? Number(form.heightCm) : null,
      weightKg: form.weightKg ? Number(form.weightKg) : null,
      ftpWatts: form.ftpWatts ? Number(form.ftpWatts) : null,
      hrMaxBpm: form.hrMaxBpm ? Number(form.hrMaxBpm) : null,
      vo2Max: form.vo2Max ? Number(form.vo2Max) : null,
    });
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
            Last Zone Update: {formatLastZoneUpdate(settings.lastZoneUpdateEpochSeconds)}
          </div>
        </div>
      </div>

      <div className="mt-6 grid gap-4 sm:grid-cols-2">
        <MetricCard
          icon={Zap}
          iconBg="bg-yellow-400/20"
          iconColor="text-yellow-400"
          label="Functional Threshold Power"
          value={settings.ftpWatts ? `${settings.ftpWatts} Watts` : '—'}
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
          value={settings.hrMaxBpm ? `${settings.hrMaxBpm} BPM` : '—'}
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

      <button
        className="mt-6 w-full flex items-center justify-center gap-2 bg-slate-800 border border-white/10 text-slate-300 font-semibold rounded-xl py-3 text-sm uppercase tracking-wider hover:bg-slate-700 transition disabled:opacity-60 disabled:cursor-not-allowed"
        onClick={handleSave}
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
  onChange: (e: React.ChangeEvent<HTMLInputElement>) => void;
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
  icon: React.ComponentType<{ size?: number; className?: string }>;
  iconBg: string;
  iconColor: string;
  label: string;
  value: string;
  editable?: boolean;
  inputValue?: string;
  onChange?: (e: React.ChangeEvent<HTMLInputElement>) => void;
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
