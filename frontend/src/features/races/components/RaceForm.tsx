import type { ReactNode } from 'react';
import { useEffect, useMemo, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { useDialogFocusTrap } from '../../../lib/useDialogFocusTrap';
import { createRace, updateRace } from '../api/races';
import type { Race, RaceDiscipline, RacePriority } from '../types';

type RaceFormProps = {
  apiBaseUrl: string;
  race: Race | null;
  onCancel: () => void;
  onSaved: () => void;
};

type RaceDraft = {
  date: string;
  name: string;
  distanceKm: string;
  discipline: RaceDiscipline;
  priority: RacePriority;
};

const defaultDraft: RaceDraft = {
  date: '',
  name: '',
  distanceKm: '',
  discipline: 'road',
  priority: 'B',
};

export function RaceForm({ apiBaseUrl, race, onCancel, onSaved }: RaceFormProps) {
  const { t } = useTranslation();
  const [draft, setDraft] = useState<RaceDraft>(defaultDraft);
  const [isSaving, setIsSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const dialogRef = useRef<HTMLElement | null>(null);
  const cancelButtonRef = useRef<HTMLButtonElement>(null);

  useDialogFocusTrap(true, dialogRef, cancelButtonRef);

  useEffect(() => {
    if (!race) {
      setDraft(defaultDraft);
      setError(null);
      return;
    }

    setDraft({
      date: race.date,
      name: race.name,
      distanceKm: String(Math.round(race.distanceMeters / 1000)),
      discipline: race.discipline,
      priority: race.priority,
    });
    setError(null);
  }, [race]);

  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape' && !isSaving) {
        onCancel();
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => {
      window.removeEventListener('keydown', handleKeyDown);
    };
  }, [isSaving, onCancel]);

  const handleRequestClose = () => {
    if (isSaving) {
      return;
    }

    onCancel();
  };

  const parsedDistanceMeters = useMemo(() => {
    const value = Number(draft.distanceKm);
    return Number.isFinite(value) && value > 0 ? Math.round(value * 1000) : null;
  }, [draft.distanceKm]);

  const isValid = draft.date.length > 0 && draft.name.trim().length > 0 && parsedDistanceMeters !== null;

  const handleSave = async () => {
    if (!isValid || parsedDistanceMeters === null) {
      return;
    }

    setIsSaving(true);
    setError(null);

    try {
      const payload = {
        date: draft.date,
        name: draft.name.trim(),
        distanceMeters: parsedDistanceMeters,
        discipline: draft.discipline,
        priority: draft.priority,
      };

      if (race) {
        await updateRace(apiBaseUrl, race.raceId, payload);
      } else {
        await createRace(apiBaseUrl, payload);
      }

      onSaved();
    } catch (err) {
      setError(err instanceof Error ? err.message : t('races.saveError'));
    } finally {
      setIsSaving(false);
    }
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-[#070b12]/82 px-4 py-6 backdrop-blur-sm" onClick={handleRequestClose}>
      <section
        ref={dialogRef}
        role="dialog"
        aria-modal="true"
        aria-labelledby="race-form-title"
        tabIndex={-1}
        className="w-full max-w-2xl rounded-[1.75rem] border border-white/8 bg-[linear-gradient(180deg,rgba(23,18,13,0.98),rgba(14,11,9,0.95))] p-6 shadow-[0_22px_80px_rgba(0,0,0,0.34)]"
        onClick={(event) => event.stopPropagation()}
      >
        <div className="flex items-start justify-between gap-4">
          <div>
            <p className="text-[10px] font-black uppercase tracking-[0.3em] text-[#d7b37b]">{t('races.editorEyebrow')}</p>
            <h2 id="race-form-title" className="mt-2 text-2xl font-black uppercase tracking-tight text-white">
              {race ? t('races.editTitle') : t('races.createTitle')}
            </h2>
            <p className="mt-3 max-w-md text-sm leading-7 text-slate-300">{t('races.formDescription')}</p>
          </div>
        </div>

        <div className="mt-6 grid gap-4 md:grid-cols-2">
          <Field label={t('races.nameLabel')}>
            <input
              value={draft.name}
              onChange={(event) => setDraft((current) => ({ ...current, name: event.target.value }))}
              className="w-full rounded-xl border border-white/10 bg-white/5 px-4 py-3 text-sm text-white outline-none transition focus:border-[#f2c98e]/45"
              placeholder={t('races.namePlaceholder')}
            />
          </Field>

          <Field label={t('races.dateLabel')}>
            <input
              type="date"
              value={draft.date}
              onChange={(event) => setDraft((current) => ({ ...current, date: event.target.value }))}
              className="w-full rounded-xl border border-white/10 bg-white/5 px-4 py-3 text-sm text-white outline-none transition focus:border-[#f2c98e]/45"
            />
          </Field>

          <Field label={t('races.distanceLabel')}>
            <input
              type="number"
              min="1"
              step="1"
              value={draft.distanceKm}
              onChange={(event) => setDraft((current) => ({ ...current, distanceKm: event.target.value }))}
              className="w-full rounded-xl border border-white/10 bg-white/5 px-4 py-3 text-sm text-white outline-none transition focus:border-[#f2c98e]/45"
              placeholder="120"
            />
          </Field>

          <Field label={t('races.disciplineLabel')}>
            <select
              value={draft.discipline}
              onChange={(event) => setDraft((current) => ({ ...current, discipline: event.target.value as RaceDiscipline }))}
              className="w-full rounded-xl border border-white/10 bg-white/5 px-4 py-3 text-sm text-white outline-none transition focus:border-[#f2c98e]/45"
            >
              <option value="road">{t('races.discipline.road')}</option>
              <option value="mtb">{t('races.discipline.mtb')}</option>
              <option value="gravel">{t('races.discipline.gravel')}</option>
              <option value="cyclocross">{t('races.discipline.cyclocross')}</option>
              <option value="timetrial">{t('races.discipline.timetrial')}</option>
            </select>
          </Field>
        </div>

        <div className="mt-5">
          <p className="mb-2 block text-xs font-medium uppercase tracking-wider text-slate-400">{t('races.priorityLabelTitle')}</p>
          <div className="flex flex-wrap gap-3">
            {(['A', 'B', 'C'] as const).map((priority) => {
              const active = draft.priority === priority;

              return (
                <button
                  key={priority}
                  type="button"
                  onClick={() => setDraft((current) => ({ ...current, priority }))}
                  aria-pressed={active}
                  className={[
                    'rounded-full border px-4 py-2 text-sm font-bold uppercase tracking-[0.18em] transition',
                    active
                      ? 'border-[#f2c98e]/45 bg-[#f2c98e]/15 text-[#f7ddb4]'
                      : 'border-white/10 bg-white/5 text-slate-300 hover:border-white/20 hover:text-white',
                  ].join(' ')}
                >
                  {t('races.priorityBadge', { priority })}
                </button>
              );
            })}
          </div>
        </div>

        {error ? (
          <div className="mt-5 rounded-2xl border border-red-500/30 bg-red-500/10 px-4 py-3 text-sm text-red-300">
            {error}
          </div>
        ) : null}

        <div className="mt-6 flex flex-wrap gap-3">
          <button
            ref={cancelButtonRef}
            type="button"
            onClick={handleRequestClose}
            disabled={isSaving}
            className="rounded-full border border-white/10 bg-white/5 px-5 py-3 text-xs font-bold uppercase tracking-[0.2em] text-slate-200 transition hover:bg-white/10 hover:text-white"
          >
            {t('races.cancel')}
          </button>
          <button
            type="button"
            disabled={!isValid || isSaving}
            onClick={() => { void handleSave(); }}
            className="rounded-full bg-[#f2c98e] px-5 py-3 text-xs font-black uppercase tracking-[0.2em] text-slate-950 transition hover:bg-[#f5d5a5] disabled:cursor-not-allowed disabled:opacity-60"
          >
            {isSaving ? t('races.saving') : race ? t('races.saveRace') : t('races.addRace')}
          </button>
        </div>
      </section>
    </div>
  );
}

function Field({ label, children }: { label: string; children: ReactNode }) {
  return (
    <label className="block">
      <span className="mb-2 block text-xs font-medium uppercase tracking-wider text-slate-400">{label}</span>
      {children}
    </label>
  );
}
