import { ChevronLeft, ChevronRight } from 'lucide-react';
import { useTranslation } from 'react-i18next';

type WorkoutHistoryPaginationProps = {
  weekLabel: string;
  canGoToNewerWeek: boolean;
  onOlderWeek: () => void;
  onNewerWeek: () => void;
};

export function WorkoutHistoryPagination({
  weekLabel,
  canGoToNewerWeek,
  onOlderWeek,
  onNewerWeek,
}: WorkoutHistoryPaginationProps) {
  const { t } = useTranslation();

  return (
    <div className="mb-4 flex items-center justify-between gap-3 rounded-xl border border-white/10 bg-black/20 px-3 py-2 text-sm text-slate-300">
      <button
        type="button"
        className="flex h-9 w-9 items-center justify-center rounded-lg text-slate-300 transition hover:bg-white/5 hover:text-white"
        aria-label={t('coach.olderWeek')}
        onClick={onOlderWeek}
      >
        <ChevronLeft size={18} />
      </button>
      <span className="text-center text-xs font-semibold uppercase tracking-[0.18em] text-slate-400">
        {weekLabel}
      </span>
      <button
        type="button"
        className="flex h-9 w-9 items-center justify-center rounded-lg text-slate-300 transition hover:bg-white/5 hover:text-white disabled:cursor-not-allowed disabled:opacity-30"
        aria-label={t('coach.newerWeek')}
        disabled={!canGoToNewerWeek}
        onClick={onNewerWeek}
      >
        <ChevronRight size={18} />
      </button>
    </div>
  );
}
