import {useTranslation} from 'react-i18next';

import type {CompletedWorkoutSummary} from '../../intervals/types';

type WorkoutSummarySectionProps = {
  isLoading: boolean;
  summary: CompletedWorkoutSummary | null;
  summaryError: string | null;
};

export function WorkoutSummarySection({
  isLoading,
  summary,
  summaryError,
}: WorkoutSummarySectionProps) {
  const {t} = useTranslation();

  return (
    <div className="rounded-2xl border border-white/6 bg-[#171a1d] p-4">
      <p className="text-[10px] font-black uppercase tracking-[0.24em] text-slate-500">{t('calendar.workoutSummary')}</p>
      {isLoading ? (
        <p className="mt-4 text-sm text-slate-400">{t('calendar.loadingWorkoutSummary')}</p>
      ) : summary ? (
        <>
          <div className="mt-4 whitespace-pre-wrap text-sm leading-7 text-slate-100">{summary.text}</div>
          {summary.provider || summary.model ? (
            <p className="mt-4 text-xs text-slate-500">
              {[summary.provider, summary.model].filter(Boolean).join(' / ')}
            </p>
          ) : null}
        </>
      ) : summaryError ? (
        <p className="mt-4 text-sm text-amber-100">{summaryError}</p>
      ) : (
        <p className="mt-4 text-sm text-slate-400">{t('calendar.workoutSummaryNotReady')}</p>
      )}
    </div>
  );
}
