import type { ReactNode } from 'react';
import { useMemo, useState } from 'react';
import { CalendarDays, Flag, Plus, Trophy } from 'lucide-react';
import { useTranslation } from 'react-i18next';

import type { Race } from '../types';
import { deleteRace } from '../api/races';
import { useRaces } from '../hooks/useRaces';
import { formatRaceDate } from '../utils';
import { invalidateCalendarCache } from '../../calendar/hooks/useCalendarData';
import { RaceCard } from './RaceCard';
import { RaceForm } from './RaceForm';

type RacesPageLayoutProps = {
  apiBaseUrl: string;
};

export function RacesPageLayout({ apiBaseUrl }: RacesPageLayoutProps) {
  const { t, i18n } = useTranslation();
  const locale = i18n.resolvedLanguage ?? i18n.language ?? 'en';
  const { upcomingRaces, completedRaces, isLoading, error, refresh } = useRaces({ apiBaseUrl });
  const [editingRace, setEditingRace] = useState<Race | null>(null);
  const [isCreating, setIsCreating] = useState(false);
  const [deletingRaceId, setDeletingRaceId] = useState<string | null>(null);
  const [deleteError, setDeleteError] = useState<string | null>(null);

  const activeRace = useMemo(() => (isCreating ? null : editingRace), [editingRace, isCreating]);

  async function handleDelete(race: Race) {
    if (deletingRaceId !== null) return;
    if (!window.confirm(t('races.deleteConfirm', { name: race.name }))) return;
    setDeletingRaceId(race.raceId);
    setDeleteError(null);
    try {
      await deleteRace(apiBaseUrl, race.raceId);
      invalidateCalendarCache();
      await refresh();
    } catch {
      setDeleteError(t('races.deleteError'));
    } finally {
      setDeletingRaceId(null);
    }
  }

  const isEditorOpen = isCreating || editingRace !== null;
  const nextRace = upcomingRaces[0] ?? null;

  return (
    <section className="space-y-6">
      <div className="space-y-6">
          <section className="overflow-hidden rounded-[1.9rem] border border-white/8 bg-[radial-gradient(circle_at_top_left,rgba(242,201,142,0.22),transparent_28%),radial-gradient(circle_at_85%_20%,rgba(120,95,64,0.2),transparent_22%),linear-gradient(180deg,rgba(19,16,13,0.98),rgba(12,14,17,0.94))] p-6 shadow-[0_24px_80px_rgba(0,0,0,0.35)] md:p-8">
            <div className="flex flex-col gap-6 md:flex-row md:items-end md:justify-between">
              <div>
                <p className="text-[10px] font-black uppercase tracking-[0.35em] text-[#d7b37b]">{t('races.eyebrow')}</p>
                <h2 className="mt-2 text-3xl font-black uppercase tracking-tight text-white md:text-4xl">{t('races.title')}</h2>
                <p className="mt-3 max-w-2xl text-sm leading-7 text-slate-300">{t('races.description')}</p>
              </div>
              <button
                type="button"
                onClick={() => {
                  setEditingRace(null);
                  setIsCreating(true);
                }}
                className="inline-flex items-center justify-center gap-2 rounded-full bg-[#f2c98e] px-5 py-3 text-sm font-black uppercase tracking-[0.18em] text-slate-950 transition hover:bg-[#f5d5a5]"
              >
                <Plus size={16} />
                {t('races.addRace')}
              </button>
            </div>

            <div className="mt-6 grid gap-3 md:grid-cols-3">
              <OverviewPill
                icon={<Flag size={15} />}
                label={t('races.upcomingMetric')}
                value={String(upcomingRaces.length)}
                accent="text-[#f2c98e]"
              />
              <OverviewPill
                icon={<Trophy size={15} />}
                label={t('races.completedMetric')}
                value={String(completedRaces.length)}
                accent="text-slate-100"
              />
              <OverviewPill
                icon={<CalendarDays size={15} />}
                label={t('races.nextRaceMetric')}
                value={nextRace ? formatRaceDate(nextRace.date, locale) : t('races.noNextRace')}
                accent="text-[#8fe8a4]"
              />
            </div>
          </section>

        {deleteError ? (
          <StatePanel tone="error">{deleteError}</StatePanel>
        ) : null}
        {isLoading ? (
          <StatePanel tone="neutral">{t('races.loading')}</StatePanel>
        ) : error ? (
          <StatePanel tone="error">{t('races.loadError', { message: error })}</StatePanel>
        ) : (
          <>
            <RaceSection
              title={t('races.upcomingTitle')}
              races={upcomingRaces}
              onEdit={setEditingRace}
              onDelete={handleDelete}
              isDeletingRace={(race) => deletingRaceId === race.raceId}
              emptyLabel={t('races.noUpcoming')}
            />
            <RaceSection
              title={t('races.completedTitle')}
              races={completedRaces}
              onEdit={setEditingRace}
              emptyLabel={t('races.noCompleted')}
            />
          </>
        )}
      </div>

      {isEditorOpen ? (
          <RaceForm
            apiBaseUrl={apiBaseUrl}
            race={activeRace}
            onCancel={() => {
              setEditingRace(null);
              setIsCreating(false);
            }}
            onSaved={() => {
              invalidateCalendarCache();
              setEditingRace(null);
              setIsCreating(false);
              void refresh();
            }}
          />
      ) : null}
    </section>
  );
}

function RaceSection({
  title,
  races,
  onEdit,
  onDelete,
  isDeletingRace,
  emptyLabel,
}: {
  title: string;
  races: Race[];
  onEdit: (race: Race) => void;
  onDelete?: (race: Race) => void;
  isDeletingRace?: (race: Race) => boolean;
  emptyLabel: string;
}) {
  return (
    <section className="space-y-4">
      <div className="flex items-center justify-between gap-3">
        <h3 className="text-xl font-black uppercase tracking-tight text-white">{title}</h3>
        <span className="text-xs font-semibold uppercase tracking-[0.24em] text-slate-500">{races.length}</span>
      </div>
      {races.length > 0 ? (
        <div className="grid gap-4 lg:grid-cols-2">
          {races.map((race) => (
            <RaceCard
              key={race.raceId}
              race={race}
              onEdit={onEdit}
              onDelete={onDelete}
              isDeleting={isDeletingRace?.(race) ?? false}
            />
          ))}
        </div>
      ) : (
        <StatePanel tone="neutral">{emptyLabel}</StatePanel>
      )}
    </section>
  );
}

function StatePanel({ tone, children }: { tone: 'neutral' | 'error'; children: ReactNode }) {
  const className = tone === 'error'
    ? 'border-red-400/25 bg-red-500/10 text-red-200'
    : 'border-white/10 bg-white/5 text-slate-400';

  return (
    <div className={`rounded-2xl border p-6 text-center ${className}`}>
      {children}
    </div>
  );
}

function OverviewPill({
  icon,
  label,
  value,
  accent,
}: {
  icon: ReactNode;
  label: string;
  value: string;
  accent: string;
}) {
  return (
    <div className="rounded-2xl border border-white/8 bg-black/15 px-4 py-4 backdrop-blur">
      <div className="flex items-center gap-2 text-slate-500">
        {icon}
        <span className="text-[10px] font-black uppercase tracking-[0.24em]">{label}</span>
      </div>
      <p className={`mt-3 text-sm font-black uppercase tracking-[0.08em] ${accent}`}>{value}</p>
    </div>
  );
}
