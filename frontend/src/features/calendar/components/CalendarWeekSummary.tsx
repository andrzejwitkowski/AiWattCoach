import { useTranslation } from 'react-i18next';

import type { CalendarWeekSummary as CalendarWeekSummaryType } from '../types';

type CalendarWeekSummaryProps = {
  weekNumber: number;
  summary: CalendarWeekSummaryType;
};

export function CalendarWeekSummary({ weekNumber, summary }: CalendarWeekSummaryProps) {
  const { t, i18n } = useTranslation();
  const locale = i18n.resolvedLanguage ?? i18n.language ?? 'en';

  return (
    <div className="flex flex-col gap-4 rounded-xl border border-white/5 border-l-4 border-l-[#d2ff9a] bg-[#111417] px-4 py-4 md:px-5 xl:flex-row xl:items-center xl:justify-between">
      <div className="flex flex-col gap-2 sm:flex-row sm:items-center sm:gap-4 xl:gap-6">
        <span className="text-xs font-black uppercase tracking-[0.2em] text-[#d2ff9a]">
          {t('calendar.week')} {String(weekNumber).padStart(2, '0')}
        </span>
        <div className="hidden h-6 w-px bg-white/10 xl:block" />
        <span className="text-[10px] font-bold uppercase tracking-widest text-slate-400">
          {summary.totalTss > 0 ? t('calendar.trainingLoadActive') : t('calendar.trainingLoadRecovery')}
        </span>
      </div>

      <div className="grid grid-cols-2 gap-4 sm:grid-cols-4 sm:gap-6 xl:gap-8">
        <Metric label={t('calendar.tssStatus')} value={formatInteger(summary.totalTss, locale)} detail={summary.targetTss !== null ? `/ ${formatInteger(summary.targetTss, locale)}` : t('calendar.actualOnly')} />
        <Metric label={t('calendar.energy')} value={formatInteger(summary.totalCalories, locale)} detail="kcal" />
        <Metric label={t('calendar.duration')} value={formatHours(summary.totalDurationSeconds, locale)} detail={summary.targetDurationSeconds !== null ? `/ ${formatHours(summary.targetDurationSeconds, locale)}` : t('calendar.actualOnly')} />
        <Metric label={t('calendar.distance')} value={formatDistance(summary.totalDistanceMeters, locale)} detail="km" />
      </div>
    </div>
  );
}

function Metric({ label, value, detail }: { label: string; value: string; detail: string }) {
  return (
    <div className="flex flex-col items-center sm:items-start">
      <p className="mb-0.5 text-[9px] font-bold uppercase text-slate-500">{label}</p>
      <p className="text-sm font-bold text-[#f9f9fd]">
        {value} <span className="text-[10px] text-slate-500">{detail}</span>
      </p>
    </div>
  );
}

function formatInteger(value: number, locale: string): string {
  return new Intl.NumberFormat(locale, { maximumFractionDigits: 0 }).format(value);
}

function formatHours(seconds: number, locale: string): string {
  const hours = seconds / 3600;
  const fractionDigits = seconds >= 36000 ? 0 : 1;
  return `${new Intl.NumberFormat(locale, {
    minimumFractionDigits: fractionDigits,
    maximumFractionDigits: fractionDigits,
  }).format(hours)}h`;
}

function formatDistance(meters: number, locale: string): string {
  const kilometers = meters / 1000;
  const fractionDigits = meters >= 100000 ? 0 : 1;
  return new Intl.NumberFormat(locale, {
    minimumFractionDigits: fractionDigits,
    maximumFractionDigits: fractionDigits,
  }).format(kilometers);
}
