import { BedDouble, Bike, Dumbbell, Flag, Footprints, Link2, Link2Off, Trophy, Waves } from 'lucide-react';
import { useTranslation } from 'react-i18next';

import type { CalendarDay, CalendarRaceLabel } from '../types';
import { formatDayLabel } from '../utils/dateUtils';
import { buildCompletedWorkoutPreviewBars, buildPlannedWorkoutBars, isPlannedWorkoutEvent, type WorkoutBar } from '../workoutDetails';
import { CalendarMiniChart } from './CalendarMiniChart';

type CalendarDayCellProps = {
  day: CalendarDay;
  isToday: boolean;
  onSelect?: (day: CalendarDay) => void;
};

type Tone = 'primary' | 'secondary' | 'error' | 'anaerobic' | 'muted' | 'race';
type PlannedSyncVisual = {
  borderClass: string;
  hoverBorderClass: string;
  icon: typeof Link2;
  iconClass: string;
  badgeClass: string;
  label: string;
};

type RacePriorityVisual = {
  className: string;
  label: string;
};

export function CalendarDayCell({ day, isToday, onSelect }: CalendarDayCellProps) {
  const { t, i18n } = useTranslation();
  const locale = i18n.resolvedLanguage ?? i18n.language ?? 'en';
  const raceLabel = day.labels.find((label): label is CalendarRaceLabel => label.kind === 'race') ?? null;
  const primaryActivity = day.activities[0] ?? null;
  const primaryEvent = day.events[0] ?? null;
  const primaryPlannedWorkoutEvent = primaryEvent && !primaryEvent.actualWorkout && isPlannedWorkoutEvent(primaryEvent)
    ? primaryEvent
    : null;
  const isPlannedOnly = Boolean(!primaryActivity && primaryPlannedWorkoutEvent);
  const isPredictedPlannedOnly = Boolean(isPlannedOnly && primaryPlannedWorkoutEvent?.plannedSource === 'predicted');
  const plannedSyncStatus = primaryPlannedWorkoutEvent?.syncStatus ?? null;
  const hasTraining = Boolean(primaryActivity || primaryEvent || raceLabel);
  const extraItemCount = Math.max(0, day.activities.length + day.events.length - 1);
  const title = raceLabel?.payload.name
    ?? (hasTraining
      ? buildTitle(primaryActivity, primaryEvent, {
        workout: t('calendar.workout'),
        race: t('calendar.eventRace'),
        ride: t('calendar.eventRide'),
        run: t('calendar.eventRun'),
        swim: t('calendar.eventSwim'),
        unknown: t('calendar.eventOther'),
      })
      : t('calendar.restDay'));
  const subtitle = raceLabel
    ? (raceLabel.subtitle ?? buildRaceSubtitle(raceLabel.payload, t))
    : (hasTraining
      ? buildSubtitle(primaryActivity, isPlannedOnly ? primaryPlannedWorkoutEvent : null, locale, {
        workout: t('calendar.workout'),
        race: t('calendar.eventRace'),
        ride: t('calendar.eventRide'),
        run: t('calendar.eventRun'),
        swim: t('calendar.eventSwim'),
        unknown: t('calendar.eventOther'),
      })
      : t('calendar.restDay'));
  const tone: Tone = raceLabel
    ? 'race'
    : hasTraining
      ? getTone(primaryActivity, primaryEvent)
      : 'muted';
  const bars = raceLabel
    ? buildRaceBars(raceLabel)
    : buildBars(primaryActivity, primaryPlannedWorkoutEvent);
  const Icon = raceLabel
    ? getRaceIcon(raceLabel.payload.discipline)
    : hasTraining
      ? getIcon(primaryActivity, primaryEvent)
      : BedDouble;
  const isSelectable = hasTraining && Boolean(onSelect);
  const plannedSyncVisual = isPredictedPlannedOnly ? getPlannedSyncVisual(plannedSyncStatus, t) : null;
  const racePriorityVisual = raceLabel ? getRacePriorityVisual(raceLabel.payload.priority) : null;

  const baseClassName = [
    'flex min-h-[160px] w-full flex-col gap-3 rounded-xl border p-3 text-left transition-colors md:min-h-[168px] md:p-3.5',
    hasTraining
      ? raceLabel
        ? 'bg-[linear-gradient(180deg,rgba(34,24,16,0.96),rgba(18,14,10,0.94))] border-[#cda56b]/30 shadow-[0_0_0_1px_rgba(205,165,107,0.08)]'
        : plannedSyncVisual
          ? `bg-[#1d2024] ${plannedSyncVisual.borderClass}`
          : 'bg-[#1d2024] border-white/5'
      : 'bg-[#1d2024]/85 border-white/5 opacity-60',
    isToday ? 'ring-1 ring-[#d2ff9a]/40 shadow-[0_0_0_1px_rgba(210,255,154,0.15)]' : '',
    isSelectable
      ? raceLabel
        ? 'cursor-pointer hover:border-[#e2ba7d]/45 hover:bg-[linear-gradient(180deg,rgba(39,28,19,0.98),rgba(22,17,13,0.96))]'
        : plannedSyncVisual
          ? `cursor-pointer hover:bg-[#20242a] ${plannedSyncVisual.hoverBorderClass}`
          : 'cursor-pointer hover:border-[#d2ff9a]/25 hover:bg-[#20242a]'
      : 'cursor-default',
  ].join(' ');

  const content = (
    <>
      <div className="flex items-start justify-between gap-2">
        <span className="text-[10px] font-bold text-slate-500">{formatDayLabel(day.date, locale)}</span>
        <div className="flex items-center gap-2">
          {racePriorityVisual ? (
            <span className={racePriorityVisual.className}>{racePriorityVisual.label}</span>
          ) : null}
          {plannedSyncVisual ? (
            <span
              className={plannedSyncVisual.badgeClass}
              title={plannedSyncVisual.label}
              aria-label={plannedSyncVisual.label}
              data-testid="planned-sync-status"
            >
              <plannedSyncVisual.icon className={plannedSyncVisual.iconClass} size={12} />
            </span>
          ) : null}
          <Icon className={iconColorClass(tone)} size={14} />
        </div>
      </div>

      {hasTraining ? (
        <div className="mt-auto">
          <CalendarMiniChart bars={bars} tone={tone} />
          {raceLabel ? (
            <div className="mb-2 flex flex-wrap items-center gap-2">
              <p className="text-[10px] font-bold uppercase tracking-[0.18em] text-[#f2c98e]">
                {t('calendar.raceDay')}
              </p>
              <p className="text-[10px] font-semibold uppercase tracking-[0.18em] text-[#d7b37b]">
                {mapRaceDiscipline(raceLabel.payload.discipline, t)}
              </p>
            </div>
          ) : isPlannedOnly ? (
            <div className="mb-2 flex flex-wrap gap-2">
              <p className="text-[10px] font-bold uppercase tracking-[0.18em] text-[#00e3fd]">
                {t('calendar.plannedWorkout')}
              </p>
              {plannedSyncVisual ? (
                <p className={`text-[10px] font-bold uppercase tracking-[0.18em] ${plannedSyncVisual.iconClass}`}>
                  {plannedSyncVisual.label}
                </p>
              ) : null}
              {plannedSyncStatus === 'modified' ? (
                <p className="text-[10px] font-bold uppercase tracking-[0.18em] text-[#ffd7a1]">
                  {t('calendar.modified')}
                </p>
              ) : null}
            </div>
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

function getRaceIcon(discipline: CalendarRaceLabel['payload']['discipline']) {
  return discipline === 'timetrial' ? Flag : Trophy;
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

function buildRaceBars(raceLabel: CalendarRaceLabel): Array<number | WorkoutBar> {
  const distanceKm = Math.max(1, Math.round(raceLabel.payload.distanceMeters / 1000));
  const peak = Math.min(95, Math.max(50, Math.round(distanceKm / 2)));

  return [
    { height: Math.max(35, peak - 22), color: '#f0d39b', widthUnits: 2 },
    { height: peak, color: '#d49c45', widthUnits: 1 },
    { height: Math.max(40, peak - 8), color: '#8d5d23', widthUnits: 1 },
  ];
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

function buildRaceSubtitle(
  race: CalendarRaceLabel['payload'],
  t: ReturnType<typeof useTranslation>['t'],
): string {
  return `${Math.round(race.distanceMeters / 1000)} km • ${t('calendar.priorityLabel', { priority: race.priority })}`;
}

function mapRaceDiscipline(
  discipline: CalendarRaceLabel['payload']['discipline'],
  t: ReturnType<typeof useTranslation>['t'],
): string {
  switch (discipline) {
    case 'road':
      return t('calendar.raceDisciplineRoad');
    case 'mtb':
      return t('calendar.raceDisciplineMtb');
    case 'gravel':
      return t('calendar.raceDisciplineGravel');
    case 'cyclocross':
      return t('calendar.raceDisciplineCyclocross');
    case 'timetrial':
      return t('calendar.raceDisciplineTimetrial');
  }
}

function getRacePriorityVisual(priority: CalendarRaceLabel['payload']['priority']): RacePriorityVisual {
  switch (priority) {
    case 'A':
      return {
        className: 'rounded-full border border-[#e9c98b]/35 bg-[#e9c98b]/12 px-2.5 py-1 text-[10px] font-bold uppercase tracking-[0.18em] text-[#f6deb1]',
        label: 'A',
      };
    case 'B':
      return {
        className: 'rounded-full border border-[#b6b0a6]/30 bg-[#b6b0a6]/10 px-2.5 py-1 text-[10px] font-bold uppercase tracking-[0.18em] text-[#d8d3cb]',
        label: 'B',
      };
    case 'C':
    default:
      return {
        className: 'rounded-full border border-[#9c6840]/30 bg-[#9c6840]/10 px-2.5 py-1 text-[10px] font-bold uppercase tracking-[0.18em] text-[#d7b08d]',
        label: 'C',
      };
  }
}

function iconColorClass(tone: Tone): string {
  switch (tone) {
    case 'race':
      return 'text-[#f2c98e]';
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

function getPlannedSyncVisual(
  syncStatus: CalendarDay['events'][number]['syncStatus'] | null,
  t: ReturnType<typeof useTranslation>['t'],
): PlannedSyncVisual {
  switch (syncStatus) {
    case 'synced':
      return {
        borderClass: 'border-[#80d998]/55 shadow-[0_0_0_1px_rgba(128,217,152,0.08)]',
        hoverBorderClass: 'hover:border-[#9af0af]/65',
        icon: Link2,
        iconClass: 'text-[#8fe8a4]',
        badgeClass: 'inline-flex h-5 w-5 items-center justify-center rounded-full border border-[#80d998]/35 bg-[#80d998]/10',
        label: t('calendar.synced'),
      };
    case 'modified':
    case 'failed':
    case 'pending':
    case 'unsynced':
    default:
      return {
        borderClass: 'border-[#b9b082]/50 shadow-[0_0_0_1px_rgba(185,176,130,0.08)]',
        hoverBorderClass: 'hover:border-[#d0c792]/65',
        icon: Link2Off,
        iconClass: 'text-[#d8ce9c]',
        badgeClass: 'inline-flex h-5 w-5 items-center justify-center rounded-full border border-[#b9b082]/35 bg-[#b9b082]/10',
        label: t('calendar.notSynced'),
      };
  }
}
