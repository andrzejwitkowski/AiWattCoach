import { BedDouble, Pencil } from 'lucide-react';
import { useTranslation } from 'react-i18next';

import type { PlannedRestDay } from '../types';
import { countPlannedRestDays, formatPlannedRestRange } from '../utils';

type PlannedRestDayCardProps = {
  entry: PlannedRestDay;
  locale: string;
  onEdit: (entry: PlannedRestDay) => void;
};

export function PlannedRestDayCard({ entry, locale, onEdit }: PlannedRestDayCardProps) {
  const { t } = useTranslation();
  const dayCount = countPlannedRestDays(entry);
  const title = entry.title?.trim() || t('plannedRestDays.defaultTitle');

  return (
    <article className="rounded-[1.4rem] border border-violet-400/20 bg-[linear-gradient(180deg,rgba(34,24,58,0.92),rgba(16,18,28,0.94))] p-5 shadow-[0_18px_50px_rgba(0,0,0,0.28)]">
      <div className="flex items-start justify-between gap-4">
        <div className="min-w-0">
          <div className="flex items-center gap-2 text-violet-200">
            <BedDouble className="h-4 w-4 shrink-0" />
            <p className="text-[10px] font-black uppercase tracking-[0.28em] text-violet-300/80">
              {t('plannedRestDays.cardEyebrow')}
            </p>
          </div>
          <h3 className="mt-2 truncate text-xl font-bold text-white">{title}</h3>
          <p className="mt-1 text-sm text-slate-300">{formatPlannedRestRange(entry, locale)}</p>
          <p className="mt-2 text-xs uppercase tracking-[0.18em] text-slate-500">
            {t('plannedRestDays.dayCount', { count: dayCount })}
          </p>
          {entry.note ? (
            <p className="mt-3 line-clamp-3 text-sm leading-6 text-slate-400">{entry.note}</p>
          ) : null}
        </div>

        <button
          type="button"
          onClick={() => onEdit(entry)}
          className="inline-flex shrink-0 items-center gap-2 rounded-full border border-white/10 px-3 py-2 text-xs font-bold uppercase tracking-[0.16em] text-slate-200 transition hover:border-violet-300/40 hover:text-violet-100"
        >
          <Pencil className="h-3.5 w-3.5" />
          {t('plannedRestDays.edit')}
        </button>
      </div>
    </article>
  );
}
