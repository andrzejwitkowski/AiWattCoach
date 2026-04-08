import { BedDouble, Bike, Dumbbell, Footprints, Waves } from 'lucide-react';
import { useTranslation } from 'react-i18next';

import type { CalendarDay } from '../types';
import { formatDayLabel } from '../utils/dateUtils';
import { buildCompletedWorkoutPreviewBars, buildPlannedWorkoutBars, isPlannedWorkoutEvent, type WorkoutBar } from '../workoutDetails';
import { CalendarMiniChart } from './CalendarMiniChart';

type CalendarDayCellProps = {
  day: CalendarDay;
  isToday: boolean;
  onSelect?: (day: CalendarDay) => void;
};

type Tone = 'primary' | 'secondary' | 'error' | 'anaerobic' | 'muted';

export function CalendarDayCell({ day, isToday, onSelect }: CalendarDayCellProps) {
  const { t, i18n } = useTranslation();
  const locale = i18n.resolvedLanguage ?? i18n.language ?? 'en';
  const primaryActivity = day.activities[0] ?? null;
  const primaryEvent = day.events[0] ?? null;
  const primaryPlannedWorkoutEvent = primaryEvent && !primaryEvent.actualWorkout && isPlannedWorkoutEvent(primaryEvent)
    ? primaryEvent
    : null;
  const isPlannedOnly = Boolean(!primaryActivity && primaryPlannedWorkoutEvent);
  const hasTraining = Boolean(primaryActivity || primaryEvent);
  const extraItemCount = Math.max(0, day.activities.length + day.events.length - 1);
  const title = hasTraining
    ? buildTitle(primaryActivity, primaryEvent, {
      workout: t('calendar.workout'),
      race: t('calendar.eventRace'),
      ride: t('calendar.eventRide'),
      run: t('calendar.eventRun'),
      swim: t('calendar.eventSwim'),
      unknown: t('calendar.eventOther'),
    })
    : t('calendar.restDay');
  const subtitle = hasTraining
    ? buildSubtitle(primaryActivity, isPlannedOnly ? primaryPlannedWorkoutEvent : null, locale, {
      workout: t('calendar.workout'),
      race: t('calendar.eventRace'),
      ride: t('calendar.eventRide'),
      run: t('calendar.eventRun'),
      swim: t('calendar.eventSwim'),
      unknown: t('calendar.eventOther'),
    })
    : t('calendar.restDay');
  const tone: Tone = hasTraining
    ? getTone(primaryActivity, primaryEvent)
    : 'muted';
  const bars = buildBars(primaryActivity, primaryPlannedWorkoutEvent);
  const Icon = hasTraining
    ? getIcon(primaryActivity, primaryEvent)
    : BedDouble;
  const isSelectable = hasTraining && Boolean(onSelect);

  const baseClassName = [
    'flex min-h-[160px] w-full flex-col gap-3 rounded-xl border p-3 text-left transition-colors md:min-h-[168px] md:p-3.5',
    hasTraining ? 'bg-[#1d2024] border-white/5' : 'bg-[#1d2024]/85 border-white/5 opacity-60',
    isToday ? 'ring-1 ring-[#d2ff9a]/40 shadow-[0_0_0_1px_rgba(210,255,154,0.15)]' : '',
    isSelectable ? 'cursor-pointer hover:border-[#d2ff9a]/25 hover:bg-[#20242a]' : 'cursor-default',
  ].join(' ');

  const content = (
    <>
      <div className="flex items-start justify-between gap-2">
        <span className="text-[10px] font-bold text-slate-500">{formatDayLabel(day.date, locale)}</span>
        <Icon className={iconColorClass(tone)} size={14} />
      </div>

      {hasTraining ? (
        <div className="mt-auto">
          <CalendarMiniChart bars={bars} tone={tone} />
          {isPlannedOnly ? (
            <p className="mb-2 text-[10px] font-bold uppercase tracking-[0.18em] text-[#00e3fd]">
              {t('calendar.plannedWorkout')}
            </p>
          ) : null}
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
    </>
  );

  if (!isSelectable) {
    return <div className={baseClassName}>{content}</div>;
  }

  return (
    <button
      type="button"
      onClick={() => onSelect?.(day)}
      className={baseClassName}
    >
      {content}
    </button>
  );
}

function buildTitle(
  dayActivity: CalendarDay['activities'][number] | null,
  dayEvent: CalendarDay['events'][number] | null,
  labels: {
    workout: string;
    race: string;
    ride: string;
    run: string;
    swim: string;
    unknown: string;
  },
): string {
  if (dayActivity) {
    return dayActivity.name ?? mapActivityType(dayActivity.activityType, labels);
  }

  if (dayEvent) {
    return dayEvent.name ?? mapEventCategory(dayEvent.category, labels);
  }

  return labels.unknown;
}

function buildSubtitle(
  dayActivity: CalendarDay['activities'][number] | null,
  dayEvent: CalendarDay['events'][number] | null,
  locale: string,
  labels: {
    workout: string;
    race: string;
    ride: string;
    run: string;
    swim: string;
    unknown: string;
  },
): string {
  const eventSummary = dayEvent?.eventDefinition.summary ?? null;
  const durationSeconds = dayActivity?.movingTimeSeconds ?? eventSummary?.totalDurationSeconds ?? 0;
  const durationMinutes = durationSeconds > 0
    ? new Intl.NumberFormat(locale, { style: 'unit', unit: 'minute', unitDisplay: 'short', maximumFractionDigits: 0 }).format(Math.round(durationSeconds / 60))
    : null;
  const estimatedTss = eventSummary?.estimatedTrainingStressScore;
  const tss = dayActivity?.metrics.trainingStressScore
    ?? (estimatedTss !== null && estimatedTss !== undefined
      ? Math.round(estimatedTss)
      : null);

  if (durationMinutes && tss !== null) {
    return `${durationMinutes} • ${tss} TSS`;
  }

  if (durationMinutes) {
    return durationMinutes;
  }

  if (tss !== null) {
    return `${tss} TSS`;
  }

  if (dayActivity) {
    return mapActivityType(dayActivity.activityType, labels);
  }

  return mapEventCategory(dayEvent?.category, labels);
}

function mapEventCategory(
  category: string | null | undefined,
  labels: {
    workout: string;
    race: string;
    ride: string;
    run: string;
    swim: string;
    unknown: string;
  },
): string {
  switch ((category ?? '').toUpperCase()) {
    case 'WORKOUT':
      return labels.workout;
    case 'RACE':
      return labels.race;
    case 'RIDE':
      return labels.ride;
    case 'RUN':
      return labels.run;
    case 'SWIM':
      return labels.swim;
    default:
      return labels.unknown;
  }
}

function mapActivityType(
  activityType: string | null | undefined,
  labels: {
    workout: string;
    race: string;
    ride: string;
    run: string;
    swim: string;
    unknown: string;
  },
): string {
  const normalized = (activityType ?? '').toLowerCase();

  if (normalized.includes('swim')) {
    return labels.swim;
  }

  if (normalized.includes('run')) {
    return labels.run;
  }

  if (normalized.includes('ride') || normalized.includes('bike') || normalized.includes('cycle')) {
    return labels.ride;
  }

  if (normalized.includes('race')) {
    return labels.race;
  }

  if (normalized.includes('workout') || normalized.includes('strength') || normalized.includes('training')) {
    return labels.workout;
  }

  return labels.unknown;
}

function getTone(dayActivity: CalendarDay['activities'][number] | null, dayEvent: CalendarDay['events'][number] | null): Tone {
  const normalized = dayActivity ? (dayActivity.activityType ?? '').toLowerCase() : `${dayEvent?.category ?? ''}`.toLowerCase();

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

function getIcon(dayActivity: CalendarDay['activities'][number] | null, dayEvent: CalendarDay['events'][number] | null) {
  const normalized = dayActivity ? (dayActivity.activityType ?? '').toLowerCase() : `${dayEvent?.category ?? ''}`.toLowerCase();

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

function buildBars(dayActivity: CalendarDay['activities'][number] | null, dayEvent: CalendarDay['events'][number] | null): Array<number | WorkoutBar> {
  if (dayActivity) {
    const bars = buildCompletedWorkoutPreviewBars(dayActivity);
    if (bars.length > 0) {
      return bars;
    }
  }

  if (dayEvent) {
    const bars = buildPlannedWorkoutBars(dayEvent);
    if (bars.length > 0) {
      return bars;
    }
  }

  const tss = dayActivity?.metrics.trainingStressScore ?? 0;
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
