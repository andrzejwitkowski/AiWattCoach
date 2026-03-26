import { BedDouble, Bike, Dumbbell, Footprints, Waves } from 'lucide-react';
import { useTranslation } from 'react-i18next';

import type { CalendarDay } from '../types';
import { formatDayLabel } from '../utils/dateUtils';
import { CalendarMiniChart } from './CalendarMiniChart';

type CalendarDayCellProps = {
  day: CalendarDay;
  isToday: boolean;
};

type Tone = 'primary' | 'secondary' | 'error' | 'anaerobic' | 'muted';

export function CalendarDayCell({ day, isToday }: CalendarDayCellProps) {
  const { t, i18n } = useTranslation();
  const locale = i18n.resolvedLanguage ?? i18n.language ?? 'en';
  const primaryActivity = day.activities[0] ?? null;
  const primaryEvent = day.events[0] ?? null;
  const hasTraining = Boolean(primaryActivity || primaryEvent);
  const extraItemCount = Math.max(0, day.activities.length + day.events.length - 1);
  const title = primaryActivity?.name ?? primaryEvent?.name ?? t('calendar.restDay');
  const subtitle = hasTraining
    ? buildSubtitle(primaryActivity, primaryEvent)
    : t('calendar.restDay');
  const tone = getTone(primaryActivity?.activityType, primaryEvent?.category);
  const bars = buildBars(primaryActivity, primaryEvent);
  const Icon = getIcon(primaryActivity?.activityType, primaryEvent?.category);

  return (
    <div
      className={[
        'flex min-h-[160px] flex-col gap-3 rounded-xl border p-3 transition-colors md:min-h-[168px] md:p-3.5',
        hasTraining ? 'bg-[#1d2024] border-white/5' : 'bg-[#1d2024]/85 border-white/5 opacity-60',
        isToday ? 'ring-1 ring-[#d2ff9a]/40 shadow-[0_0_0_1px_rgba(210,255,154,0.15)]' : '',
      ].join(' ')}
    >
      <div className="flex items-start justify-between gap-2">
        <span className="text-[10px] font-bold text-slate-500">{formatDayLabel(day.date, locale)}</span>
        <Icon className={iconColorClass(tone)} size={14} />
      </div>

      {hasTraining ? (
        <div className="mt-auto">
          <CalendarMiniChart bars={bars} tone={tone} />
          <p className="truncate text-[11px] font-bold text-[#f9f9fd]">{title}</p>
          <p className="text-[10px] text-slate-500">{subtitle}</p>
          {extraItemCount > 0 ? (
            <p className="mt-2 text-[10px] font-bold uppercase tracking-widest text-[#00e3fd]">
              {t('calendar.moreItems', { count: extraItemCount })}
            </p>
          ) : null}
        </div>
      ) : (
        <div className="flex flex-1 flex-col items-center justify-center text-center">
          <BedDouble className="mb-2 text-slate-600" size={30} />
          <p className="text-[10px] font-bold uppercase tracking-widest text-slate-500">{t('calendar.restDay')}</p>
        </div>
      )}
    </div>
  );
}

function buildSubtitle(dayActivity: CalendarDay['activities'][number] | null, dayEvent: CalendarDay['events'][number] | null): string {
  const durationSeconds = dayActivity?.movingTimeSeconds ?? 0;
  const durationMinutes = durationSeconds > 0 ? `${Math.round(durationSeconds / 60)} min` : null;
  const tss = dayActivity?.metrics.trainingStressScore ?? null;

  if (durationMinutes && tss !== null) {
    return `${durationMinutes} • ${tss} TSS`;
  }

  if (durationMinutes) {
    return durationMinutes;
  }

  if (tss !== null) {
    return `${tss} TSS`;
  }

  return dayEvent?.category ?? 'Workout';
}

function getTone(activityType: string | null | undefined, category: string | null | undefined): Tone {
  const normalized = `${activityType ?? category ?? ''}`.toLowerCase();

  if (normalized.includes('swim')) {
    return 'secondary';
  }

  if (normalized.includes('run')) {
    return 'error';
  }

  if (normalized.includes('anaerobic') || normalized.includes('sprint')) {
    return 'anaerobic';
  }

  if (normalized.includes('strength')) {
    return 'muted';
  }

  return 'primary';
}

function getIcon(activityType: string | null | undefined, category: string | null | undefined) {
  const normalized = `${activityType ?? category ?? ''}`.toLowerCase();

  if (normalized.includes('swim')) {
    return Waves;
  }

  if (normalized.includes('run')) {
    return Footprints;
  }

  if (normalized.includes('strength')) {
    return Dumbbell;
  }

  return Bike;
}

function buildBars(dayActivity: CalendarDay['activities'][number] | null, dayEvent: CalendarDay['events'][number] | null): number[] {
  const intervalCount = dayEvent?.eventDefinition.intervals.length ?? 0;
  const tss = dayActivity?.metrics.trainingStressScore ?? 0;

  if (intervalCount > 0) {
    return dayEvent?.eventDefinition.intervals.map((_, index) => {
      const divisor = Math.max(1, intervalCount - 1);
      return 35 + Math.round((index / divisor) * 55);
    }) ?? [];
  }

  if (tss > 0) {
    const peak = Math.min(100, Math.max(30, tss));
    return [Math.max(20, peak - 25), peak, Math.max(25, peak - 10)];
  }

  return [35, 55, 75, 55];
}

function iconColorClass(tone: Tone): string {
  switch (tone) {
    case 'secondary':
      return 'text-[#00e3fd]';
    case 'error':
      return 'text-[#ff7351]';
    case 'anaerobic':
      return 'text-[#800020]';
    case 'muted':
      return 'text-slate-500';
    case 'primary':
    default:
      return 'text-[#d2ff9a]';
  }
}
