import type { ReactNode } from 'react';
import { useMemo, useState } from 'react';
import { BedDouble, CalendarDays, MoonStar, Plus } from 'lucide-react';
import { useTranslation } from 'react-i18next';

import type { PlannedRestDay } from '../types';
import { usePlannedRestDays } from '../hooks/usePlannedRestDays';
import { invalidateCalendarCache } from '../../calendar/hooks/useCalendarData';
import { countUniquePlannedRestCalendarDays, formatPlannedRestRange } from '../utils';
import { PlannedRestDayCard } from './PlannedRestDayCard';
import { PlannedRestDayForm } from './PlannedRestDayForm';

export function PlannedRestDaysPageLayout() {
  const { t, i18n } = useTranslation();
  const locale = i18n.resolvedLanguage ?? i18n.language ?? 'en';
  const { upcomingEntries, pastEntries, isLoading, error, refresh } = usePlannedRestDays();
  const [editingEntry, setEditingEntry] = useState<PlannedRestDay | null>(null);
  const [isCreating, setIsCreating] = useState(false);

  const activeEntry = useMemo(() => (isCreating ? null : editingEntry), [editingEntry, isCreating]);
  const isEditorOpen = isCreating || editingEntry !== null;
  const nextEntry = upcomingEntries[0] ?? null;
  const upcomingDayCount = useMemo(
    () => countUniquePlannedRestCalendarDays(upcomingEntries),
    [upcomingEntries],
  );

  const handleSaved = async () => {
    invalidateCalendarCache();
    setEditingEntry(null);
    setIsCreating(false);
    await refresh();
  };

  return (
    <section className="space-y-6">
      <section className="overflow-hidden rounded-[1.9rem] border border-white/8 bg-[radial-gradient(circle_at_top_left,rgba(167,139,250,0.22),transparent_28%),radial-gradient(circle_at_85%_20%,rgba(76,29,149,0.18),transparent_22%),linear-gradient(180deg,rgba(19,16,28,0.98),rgba(12,14,17,0.94))] p-6 shadow-[0_24px_80px_rgba(0,0,0,0.35)] md:p-8">
        <div className="flex flex-col gap-6 md:flex-row md:items-end md:justify-between">
          <div>
            <p className="text-[10px] font-black uppercase tracking-[0.35em] text-violet-300">{t('plannedRestDays.eyebrow')}</p>
            <h2 className="mt-2 text-3xl font-black uppercase tracking-tight text-white md:text-4xl">{t('plannedRestDays.title')}</h2>
            <p className="mt-3 max-w-2xl text-sm leading-7 text-slate-300">{t('plannedRestDays.description')}</p>
          </div>
          <button
            type="button"
            onClick={() => {
              setEditingEntry(null);
              setIsCreating(true);
            }}
            className="inline-flex items-center justify-center gap-2 rounded-full bg-violet-300 px-5 py-3 text-sm font-black uppercase tracking-[0.18em] text-slate-950 transition hover:bg-violet-200"
          >
            <Plus size={16} />
            {t('plannedRestDays.add')}
          </button>
        </div>

        <div className="mt-6 grid gap-3 md:grid-cols-3">
          <OverviewPill
            icon={<MoonStar size={15} />}
            label={t('plannedRestDays.upcomingBlocksMetric')}
            value={String(upcomingEntries.length)}
            accent="text-violet-200"
          />
          <OverviewPill
            icon={<BedDouble size={15} />}
            label={t('plannedRestDays.upcomingDaysMetric')}
            value={String(upcomingDayCount)}
            accent="text-slate-100"
          />
          <OverviewPill
            icon={<CalendarDays size={15} />}
            label={t('plannedRestDays.nextBlockMetric')}
            value={nextEntry ? formatPlannedRestRange(nextEntry, locale) : t('plannedRestDays.noNextBlock')}
            accent="text-[#c4b5fd]"
          />
        </div>
      </section>

      {isLoading ? (
        <StatePanel tone="neutral">{t('plannedRestDays.loading')}</StatePanel>
      ) : error ? (
        <StatePanel tone="error">{t('plannedRestDays.loadError', { message: error })}</StatePanel>
      ) : (
        <>
          <EntrySection
            title={t('plannedRestDays.upcomingTitle')}
            entries={upcomingEntries}
            locale={locale}
            emptyLabel={t('plannedRestDays.noUpcoming')}
            onEdit={setEditingEntry}
          />
          <EntrySection
            title={t('plannedRestDays.pastTitle')}
            entries={pastEntries}
            locale={locale}
            emptyLabel={t('plannedRestDays.noPast')}
            onEdit={setEditingEntry}
          />
        </>
      )}

      {isEditorOpen ? (
        <PlannedRestDayForm
          entry={activeEntry}
          onCancel={() => {
            setEditingEntry(null);
            setIsCreating(false);
          }}
          onSaved={() => {
            void handleSaved();
          }}
        />
      ) : null}
    </section>
  );
}

function EntrySection({
  title,
  entries,
  locale,
  emptyLabel,
  onEdit,
}: {
  title: string;
  entries: PlannedRestDay[];
  locale: string;
  emptyLabel: string;
  onEdit: (entry: PlannedRestDay) => void;
}) {
  return (
    <section className="space-y-4">
      <h3 className="text-sm font-black uppercase tracking-[0.24em] text-slate-400">{title}</h3>
      {entries.length === 0 ? (
        <StatePanel tone="neutral">{emptyLabel}</StatePanel>
      ) : (
        <div className="grid gap-4">
          {entries.map((entry) => (
            <PlannedRestDayCard key={entry.plannedRestDayId} entry={entry} locale={locale} onEdit={onEdit} />
          ))}
        </div>
      )}
    </section>
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
    <div className="rounded-[1.3rem] border border-white/8 bg-black/20 px-4 py-4">
      <div className={`flex items-center gap-2 text-xs font-bold uppercase tracking-[0.18em] ${accent}`}>
        {icon}
        {label}
      </div>
      <p className="mt-2 text-lg font-bold text-white">{value}</p>
    </div>
  );
}

function StatePanel({ tone, children }: { tone: 'neutral' | 'error'; children: ReactNode }) {
  return (
    <div
      className={[
        'rounded-[1.4rem] border px-5 py-4 text-sm',
        tone === 'error'
          ? 'border-rose-400/30 bg-rose-400/10 text-rose-100'
          : 'border-white/8 bg-white/5 text-slate-300',
      ].join(' ')}
    >
      {children}
    </div>
  );
}
