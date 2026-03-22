import { useState } from 'react';
import type { CyclingSettingsData, UserSettingsResponse } from '../types';
import { updateCycling } from '../api/settings';
import type { UpdateCyclingRequest } from '../types';

type CyclingSettingsCardProps = {
  settings: UserSettingsResponse;
  apiBaseUrl: string;
  onSave: () => void;
};

function Field({ label, id, value, onChange, type = 'text', placeholder }: {
  label: string;
  id: string;
  value: string | number | null;
  onChange: (v: string) => void;
  type?: string;
  placeholder?: string;
}) {
  return (
    <div>
      <label className="mb-1.5 block text-xs font-medium uppercase tracking-wider text-slate-400" htmlFor={id}>
        {label}
      </label>
      <input
        id={id}
        type={type}
        className="w-full rounded-xl border border-white/10 bg-white/5 px-4 py-2.5 text-sm text-white placeholder-slate-600 focus:border-cyan-500/50 focus:outline-none focus:ring-1 focus:ring-cyan-500/30"
        placeholder={placeholder}
        value={value ?? ''}
        onChange={(e) => onChange(e.target.value)}
      />
    </div>
  );
}

function MetricCard({ label, value, unit, accent }: { label: string; value: number | null; unit: string; accent: string }) {
  return (
    <div className={`flex flex-col items-center rounded-xl border p-4 ${accent}`}>
      <span className="text-xs font-medium uppercase tracking-wider text-slate-400">{label}</span>
      <span className="mt-1 text-2xl font-bold text-white">{value ?? '--'}</span>
      <span className="text-xs text-slate-400">{unit}</span>
    </div>
  );
}

function computeProfileAccuracy(settings: CyclingSettingsData): number {
  const fields = [
    settings.fullName,
    settings.age,
    settings.heightCm,
    settings.weightKg,
    settings.ftpWatts,
    settings.hrMaxBpm,
    settings.vo2Max,
  ];
  const filled = fields.filter((f) => f !== null && f !== undefined).length;
  return Math.round((filled / fields.length) * 100);
}

export function CyclingSettingsCard({ settings, apiBaseUrl, onSave }: CyclingSettingsCardProps) {
  const cycling = settings.cycling;
  const [form, setForm] = useState({
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

  function setField(key: keyof typeof form, value: string) {
    setForm((prev) => ({ ...prev, [key]: value }));
  }

  async function handleSave() {
    setIsSaving(true);
    try {
      const req: UpdateCyclingRequest = {};
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
  }

  const accuracy = computeProfileAccuracy(cycling);
  const lastZoneLabel = cycling.lastZoneUpdateEpochSeconds
    ? `${Math.floor((Date.now() / 1000 - cycling.lastZoneUpdateEpochSeconds) / 86400)} days ago`
    : 'Never';

  return (
    <div className="rounded-[1.5rem] border border-white/10 bg-white/5 p-6">
      <div className="mb-5">
        <h3 className="text-lg font-semibold text-white">Cycling Settings</h3>
        <p className="text-xs font-medium uppercase tracking-wider text-amber-300">USTAWIENIA KOLARSKIE</p>
        <p className="mt-2 text-sm text-slate-400">
          Physiological metrics used for training load calculations and performance analysis.
        </p>
      </div>

      <div className="mb-6 grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
        <Field label="Full Name / IME I NAZWISKO" id="full-name" value={form.fullName} onChange={(v) => setField('fullName', v)} placeholder="Alex Rivier" />
        <Field label="Age / WIEK" id="age" value={form.age} onChange={(v) => setField('age', v)} type="number" placeholder="28" />
        <Field label="Height cm / WZROST" id="height-cm" value={form.heightCm} onChange={(v) => setField('heightCm', v)} type="number" placeholder="182" />
        <Field label="Weight kg / WAGA" id="weight-kg" value={form.weightKg} onChange={(v) => setField('weightKg', v)} type="number" placeholder="74.0" />
      </div>

      {/* Profile Accuracy & Last Zone */}
      <div className="mb-6 flex items-center justify-between rounded-xl border border-white/10 bg-white/5 px-4 py-3">
        <div className="flex items-center gap-4">
          <div>
            <p className="text-xs text-slate-400">Profile Accuracy</p>
            <p className="text-sm font-semibold text-cyan-300">{accuracy}% Complete</p>
          </div>
          <div className="h-2 w-24 overflow-hidden rounded-full bg-white/10">
            <div className="h-full rounded-full bg-cyan-500" style={{ width: `${accuracy}%` }} />
          </div>
        </div>
        <div className="text-right">
          <p className="text-xs text-slate-400">Last Zone Update</p>
          <p className="text-sm font-semibold text-slate-300">{lastZoneLabel}</p>
        </div>
      </div>

      {/* Metric Cards */}
      <div className="mb-6 grid grid-cols-2 gap-4">
        <MetricCard label="FTP" value={cycling.ftpWatts} unit="Watts" accent="border-amber-500/30 bg-amber-500/10" />
        <MetricCard label="HR MAX" value={cycling.hrMaxBpm} unit="BPM" accent="border-red-500/30 bg-red-500/10" />
      </div>

      {/* VO2 Max */}
      <div className="mb-6">
        <Field label="VO2 Max / POJEMNOŚĆ TLENOWA" id="vo2-max" value={form.vo2Max} onChange={(v) => setField('vo2Max', v)} type="number" placeholder="58.0" />
      </div>

      {/* FTP and HR Max from form */}
      <div className="mb-6 grid gap-4 sm:grid-cols-2">
        <Field label="FTP / PRóg MOCY" id="ftp-watts" value={form.ftpWatts} onChange={(v) => setField('ftpWatts', v)} type="number" placeholder="280" />
        <Field label="HR MAX / TĘTNO MAKSYMALNE" id="hr-max" value={form.hrMaxBpm} onChange={(v) => setField('hrMaxBpm', v)} type="number" placeholder="192" />
      </div>

      {saveError && (
        <div className="mb-4 rounded-xl border border-red-500/30 bg-red-500/10 px-4 py-3 text-sm text-red-300">
          {saveError}
        </div>
      )}

      <button
        type="button"
        className="flex w-full items-center justify-center gap-2 rounded-xl border border-cyan-500/30 bg-cyan-500/10 px-4 py-3 text-sm font-semibold text-cyan-300 transition hover:bg-cyan-500/20 disabled:opacity-50"
        disabled={isSaving}
        onClick={() => { void handleSave(); }}
      >
        <svg className="h-4 w-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={1.5}>
          <path strokeLinecap="round" strokeLinejoin="round" d="M7.5 21L3 16.5m0 0L7.5 12M3 16.5h13.5m0-13.5L21 7.5m0 0L16.5 12M21 7.5H7.5" />
        </svg>
        {isSaving ? 'Syncing...' : 'SYNCHRONIZE BIO-METRICS'}
      </button>
    </div>
  );
}
