import { useTranslation } from 'react-i18next';

export function CalendarLoadingRow() {
  const { t } = useTranslation();

  return (
    <div className="flex min-h-[320px] items-center justify-center rounded-2xl border border-white/6 bg-[#171a1d]/80 px-6 py-10">
      <div className="flex items-center gap-4 text-sm font-semibold uppercase tracking-[0.24em] text-slate-400">
        <div aria-hidden="true" className="h-6 w-6 animate-spin rounded-full border-2 border-[#00e3fd]/60 border-t-transparent" />
        <span>{t('calendar.retrievingEvents')}</span>
      </div>
    </div>
  );
}
