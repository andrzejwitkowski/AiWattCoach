import { useTranslation } from 'react-i18next';

import { CALENDAR_WEEK_ROW_HEIGHT } from '../constants';

type CalendarLoadingRowProps = {
  status?: 'idle' | 'loading';
  showLoadingIndicator?: boolean;
};

export function CalendarLoadingRow({ status = 'loading', showLoadingIndicator = true }: CalendarLoadingRowProps) {
  const { t } = useTranslation();
  const shouldShowIndicator = status === 'loading' && showLoadingIndicator;

  return (
    <section
      className="relative flex flex-col gap-4 overflow-hidden"
      style={{ height: `${CALENDAR_WEEK_ROW_HEIGHT}px` }}
      aria-busy={shouldShowIndicator}
    >
      <div className="rounded-xl border border-white/5 border-l-4 border-l-white/10 bg-[#111417]/75 px-4 py-4 md:px-5">
        <div className="flex flex-col gap-4 xl:flex-row xl:items-center xl:justify-between">
          <div className="flex flex-col gap-2 sm:flex-row sm:items-center sm:gap-4 xl:gap-6">
            <div className="h-3 w-20 rounded-full bg-white/8" />
            <div className="hidden h-6 w-px bg-white/5 xl:block" />
            <div className="h-3 w-28 rounded-full bg-white/6" />
          </div>
          <div className="grid grid-cols-2 gap-4 sm:grid-cols-4 sm:gap-6 xl:gap-8">
            {Array.from({ length: 4 }, (_, index) => (
              <div key={index} className="flex flex-col items-center gap-2 sm:items-start">
                <div className="h-2 w-14 rounded-full bg-white/6" />
                <div className="h-3 w-16 rounded-full bg-white/8" />
              </div>
            ))}
          </div>
        </div>
      </div>

      <div className="calendar-grid gap-3">
        {Array.from({ length: 7 }, (_, index) => (
          <div
            key={index}
            className="min-h-[160px] rounded-xl border border-white/5 bg-[#1d2024]/55 md:min-h-[168px]"
          />
        ))}
      </div>

      {shouldShowIndicator ? (
        <div className="pointer-events-none absolute inset-0 flex items-center justify-center">
          <div className="flex items-center gap-3 rounded-full border border-white/10 bg-[#0f1317]/92 px-4 py-2 text-xs font-semibold uppercase tracking-[0.24em] text-slate-300 shadow-[0_12px_40px_rgba(0,0,0,0.28)]">
            <div aria-hidden="true" className="h-4 w-4 animate-spin rounded-full border-2 border-[#00e3fd]/60 border-t-transparent" />
            <span>{t('calendar.fetchingData')}</span>
          </div>
        </div>
      ) : null}
    </section>
  );
}
