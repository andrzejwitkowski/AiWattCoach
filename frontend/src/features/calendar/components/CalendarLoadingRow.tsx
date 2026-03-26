import { useTranslation } from 'react-i18next';

import { CALENDAR_WEEK_ROW_HEIGHT } from '../constants';

export function CalendarLoadingRow() {
  const { t } = useTranslation();

  return (
    <div
      className="relative overflow-hidden rounded-2xl border border-white/10 bg-[linear-gradient(180deg,rgba(23,26,29,0.88),rgba(15,18,22,0.82))] px-5 py-5 shadow-[0_18px_60px_rgba(0,0,0,0.28)] backdrop-blur-xl"
      style={{ minHeight: `${CALENDAR_WEEK_ROW_HEIGHT}px` }}
    >
      <div className="absolute inset-0 bg-[radial-gradient(circle_at_top_right,rgba(0,227,253,0.12),transparent_50%),radial-gradient(circle_at_bottom_left,rgba(210,255,154,0.1),transparent_42%)]" />
      <div className="relative flex h-full flex-col gap-4">
        <div className="flex items-center gap-3 text-xs font-black uppercase tracking-[0.24em] text-slate-300">
          <div aria-hidden="true" className="h-5 w-5 animate-spin rounded-full border-2 border-[#00e3fd]/60 border-t-transparent" />
          <span>{t('calendar.loadingWeek')}</span>
        </div>

        <div className="rounded-xl border border-white/6 bg-white/[0.03] px-4 py-4">
          <div className="mb-4 flex flex-col gap-3 xl:flex-row xl:items-center xl:justify-between">
            <div className="flex items-center gap-4">
              <div className="h-4 w-24 rounded-full bg-white/10" />
              <div className="h-4 w-px bg-white/10" />
              <div className="h-3 w-40 rounded-full bg-white/8" />
            </div>
            <div className="grid grid-cols-4 gap-4 xl:min-w-[26rem]">
              {Array.from({ length: 4 }, (_, index) => (
                <div key={index} className="space-y-2">
                  <div className="h-2 w-12 rounded-full bg-white/8" />
                  <div className="h-3 w-10 rounded-full bg-white/10" />
                </div>
              ))}
            </div>
          </div>

          <div className="grid grid-cols-7 gap-3">
            {Array.from({ length: 7 }, (_, index) => (
              <div key={index} className="rounded-2xl border border-white/6 bg-[#171a1d]/65 p-3 opacity-75">
                <div className="h-3 w-10 rounded-full bg-white/8" />
                <div className="mt-6 h-14 rounded-xl bg-white/[0.04]" />
                <div className="mt-4 h-2 w-16 rounded-full bg-white/8" />
                <div className="mt-2 h-2 w-12 rounded-full bg-white/6" />
              </div>
            ))}
          </div>
        </div>

        <p className="text-xs font-semibold uppercase tracking-[0.18em] text-slate-500">
          {t('calendar.upcomingTrainingData')}
        </p>
      </div>
    </div>
  );
}
