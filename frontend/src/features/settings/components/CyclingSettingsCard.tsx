import { useState } from 'react';
import { RefreshCw } from 'lucide-react';
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

function TextareaField({ label, id, value, onChange, placeholder, rows = 4 }: {
  label: string;
  id: string;
  value: string | null;
  onChange: (v: string) => void;
  placeholder?: string;
  rows?: number;
}) {
  return (
    <div>
      <label className="mb-1.5 block text-xs font-medium uppercase tracking-wider text-slate-400" htmlFor={id}>
        {label}
      </label>
      <textarea
        id={id}
        rows={rows}
        className="w-full rounded-xl border border-white/10 bg-white/5 px-4 py-3 text-sm text-white placeholder-slate-600 focus:border-cyan-500/50 focus:outline-none focus:ring-1 focus:ring-cyan-500/30"
        placeholder={placeholder}
        value={value ?? ''}
        onChange={(e) => onChange(e.target.value)}
      />
    </div>
  );
}

function MetricCard({ label, value, unit, accent }: { label: string; value: number | null; unit: string; accent: string }) {
  return (
    <div className={"flex flex-col items-center rounded-xl border p-4 " + accent}>
      <span className="text-xs font-medium uppercase tracking-wider text-slate-400">{label}</span>
      <span className="mt-1 text-2xl font-bold text-white">{value ?? '--'}</span>
      <span className="text-xs text-slate-400">{unit}</span>
    </div>
  );
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
    athletePrompt: cycling.athletePrompt ?? '',
    medications: cycling.medications ?? '',
    athleteNotes: cycling.athleteNotes ?? '',
  });
  const [isSaving, setIsSaving] = useState(false);
  const [saveError, setSaveError] = useState<string | null>(null);

  const accuracy = computeProfileAccuracy(cycling);

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
      req.athletePrompt = form.athletePrompt;
      req.medications = form.medications;
      req.athleteNotes = form.athleteNotes;
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
            <Field
              label="Full Name"
              id="full-name"
              value={form.fullName}
              onChange={(v) => { setForm((p) => ({ ...p, fullName: v })); setSaveError(null); }}
              placeholder="Alex Rivier"
            />
            <Field
              label="Age"
              id="age"
              type="number"
              value={form.age}
              onChange={(v) => { setForm((p) => ({ ...p, age: v })); setSaveError(null); }}
              placeholder="28"
            />
            <Field
              label="Height"
              id="height-cm"
              type="number"
              value={form.heightCm}
              onChange={(v) => { setForm((p) => ({ ...p, heightCm: v })); setSaveError(null); }}
              placeholder="182"
            />
            <Field
              label="Weight"
              id="weight-kg"
              type="number"
              value={form.weightKg}
              onChange={(v) => { setForm((p) => ({ ...p, weightKg: v })); setSaveError(null); }}
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
          label="FTP"
          value={cycling.ftpWatts}
          unit="Watts"
          accent="border-amber-500/30 bg-amber-500/10"
        />
        <MetricCard
          label="HR MAX"
          value={cycling.hrMaxBpm}
          unit="BPM"
          accent="border-red-500/30 bg-red-500/10"
        />
      </div>

      <div className="mt-6 grid gap-4 sm:grid-cols-2">
        <Field
          label="FTP (watts)"
          id="ftp-watts"
          type="number"
          value={form.ftpWatts}
          onChange={(v) => { setForm((p) => ({ ...p, ftpWatts: v })); setSaveError(null); }}
          placeholder="280"
        />
        <Field
          label="HR Max (BPM)"
          id="hr-max"
          type="number"
          value={form.hrMaxBpm}
          onChange={(v) => { setForm((p) => ({ ...p, hrMaxBpm: v })); setSaveError(null); }}
          placeholder="192"
        />
      </div>

      <div className="mt-6">
        <Field
          label="VO2 Max"
          id="vo2-max"
          type="number"
          value={form.vo2Max}
          onChange={(v) => { setForm((p) => ({ ...p, vo2Max: v })); setSaveError(null); }}
          placeholder="62.0"
        />
      </div>

      <div className="mt-8 grid gap-4 lg:grid-cols-3">
        <div className="lg:col-span-2">
          <TextareaField
            label="Athlete Prompt"
            id="athlete-prompt"
            value={form.athletePrompt}
            onChange={(v) => { setForm((p) => ({ ...p, athletePrompt: v })); setSaveError(null); }}
            placeholder="Context for the AI coach: goals, lifestyle, constraints, communication preferences."
            rows={6}
          />
        </div>
        <div className="rounded-xl border border-white/10 bg-slate-900/40 p-4 text-sm text-slate-300">
          <p className="text-[10px] uppercase tracking-widest text-slate-500">AI Context</p>
          <p className="mt-3 leading-relaxed">
            This information becomes part of the athlete profile used to build coaching context.
            Keep it factual and durable. Sensitive details may be sent to the active LLM provider
            during coach conversations.
          </p>
        </div>
      </div>

      <div className="mt-6 grid gap-4 lg:grid-cols-2">
        <TextareaField
          label="Medications"
          id="medications"
          value={form.medications}
          onChange={(v) => { setForm((p) => ({ ...p, medications: v })); setSaveError(null); }}
          placeholder="List medications, supplements, or medical factors relevant to coaching."
          rows={5}
        />
        <TextareaField
          label="Athlete Notes"
          id="athlete-notes"
          value={form.athleteNotes}
          onChange={(v) => { setForm((p) => ({ ...p, athleteNotes: v })); setSaveError(null); }}
          placeholder="Anything else the coach should always know: work schedule, sleep constraints, travel, preferences."
          rows={5}
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
