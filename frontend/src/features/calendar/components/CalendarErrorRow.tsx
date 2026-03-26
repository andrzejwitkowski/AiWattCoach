import { AlertTriangle } from 'lucide-react';
import { useTranslation } from 'react-i18next';

export function CalendarErrorRow() {
  const { t } = useTranslation();

  return (
    <div className="flex min-h-[320px] flex-col items-center justify-center rounded-2xl border border-[#ff7351]/20 bg-[#171a1d]/90 px-6 py-10 text-center">
      <AlertTriangle className="mb-4 text-[#ff7351]" size={28} />
      <p className="text-sm font-semibold uppercase tracking-[0.24em] text-slate-400">{t('calendar.rowLoadError')}</p>
      <p className="mt-3 max-w-md text-sm text-slate-500">{t('calendar.scrollToRetry')}</p>
    </div>
  );
}
