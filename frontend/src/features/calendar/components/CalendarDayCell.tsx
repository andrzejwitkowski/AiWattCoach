import { BedDouble, Bike, Dumbbell, Flag, Footprints, Link2, Link2Off, Trophy, Waves } from 'lucide-react';
import { useTranslation } from 'react-i18next';

import { buildDayItems, isInteractiveDayItem } from '../dayItems';
import { formatRaceSubtitle, mapRaceDisciplineLabel } from '../racePresentation';
import type { CalendarDay, CalendarRaceLabel } from '../types';
import { formatDayLabel } from '../utils/dateUtils';
import { buildCompletedWorkoutPreviewBars, buildPlannedWorkoutBars, isPlannedWorkoutEvent, type WorkoutBar } from '../workoutDetails';
import { CalendarMiniChart } from './CalendarMiniChart';

type CalendarDayCellProps = {
  day: CalendarDay;
  isToday: boolean;
  onSelect?: (day: CalendarDay) => void;
};

type CalendarDayEvent = CalendarDay['events'][number];

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
  const isPastDay = !isToday && day.date.getTime() < startOfDay(new Date()).getTime();
  const raceLabel = day.labels.find((label): label is CalendarRaceLabel => label.kind === 'race') ?? null;
  const raceLabels = day.labels.filter((label): label is CalendarRaceLabel => label.kind === 'race');
  const linkedRaceEventIds = new Set(
    raceLabels
      .map((label) => label.payload.linkedIntervalsEventId)
      .filter((value): value is number => value !== null),
  );
  const nonRaceEvents = linkedRaceEventIds.size > 0
    ? day.events.filter((event) => !linkedRaceEventIds.has(event.linkedIntervalsEventId ?? event.id))
    : day.events;
  const dayItems = buildDayItems(day, {
    locale,
    labels: {
      plannedWorkout: t('calendar.plannedWorkout'),
      workout: t('calendar.workout'),
    },
    t,
  });
  const interactiveDayItems = dayItems.filter(isInteractiveDayItem);
  const primaryActivity = day.activities[0] ?? null;
  const primaryCompletedActivity = interactiveDayItems.find((item): item is Extract<typeof interactiveDayItems[number], { kind: 'completed' }> => item.kind === 'completed')?.activity ?? null;
  const primaryPlannedItem = interactiveDayItems.find((item): item is Extract<typeof interactiveDayItems[number], { kind: 'planned' }> => item.kind === 'planned') ?? null;
  const matchedPlannedActivity = primaryPlannedItem?.activity ?? null;
  const hasMatchedPlannedWorkout = Boolean(primaryPlannedItem?.event.actualWorkout && matchedPlannedActivity);
  const primaryEvent = primaryPlannedItem?.event
    ?? nonRaceEvents.find((event) => Boolean(event.actualWorkout))
    ?? day.events[0]
    ?? null;
  const visibleActivity = hasMatchedPlannedWorkout
    ? matchedPlannedActivity
    : primaryPlannedItem
      ? null
      : (primaryCompletedActivity ?? primaryActivity);
  const primaryPlannedWorkoutEvent = raceLabel
    ? primaryPlannedItem?.event ?? null
    : primaryPlannedItem?.event
      ? primaryPlannedItem.event
      : primaryEvent && !primaryEvent.actualWorkout && isPlannedWorkoutEvent(primaryEvent)
        ? primaryEvent
        : null;
  const isPlannedRestDay = Boolean(primaryPlannedWorkoutEvent?.restDay);
  const isPlannedOnly = Boolean(!visibleActivity && !raceLabel && primaryPlannedWorkoutEvent);
  const isMissedPlannedOnly = Boolean(isPastDay && isPlannedOnly);
  const isPredictedPlannedOnly = Boolean(
    isPlannedOnly
      && !isPlannedRestDay
      && primaryPlannedWorkoutEvent?.plannedSource === 'predicted',
  );
  const hasCompactRacePrep = Boolean(raceLabel && primaryPlannedWorkoutEvent && !visibleActivity);
  const plannedSyncStatus = primaryPlannedWorkoutEvent?.syncStatus ?? null;
  const hasTraining = Boolean(visibleActivity || primaryEvent || raceLabel);
  const pickerVisibleItemCount = dayItems.length;
  const visibleItemCount = visibleActivity
    ? 1
    : hasCompactRacePrep
      ? 2
      : hasTraining
        ? 1
        : 0;
  const extraItemCount = Math.max(0, pickerVisibleItemCount - visibleItemCount);
  const title = raceLabel?.payload.name
    ?? (hasTraining
      ? buildTitle(visibleActivity, primaryEvent, {
        workout: t('calendar.workout'),
        race: t('calendar.eventRace'),
        ride: t('calendar.eventRide'),
        run: t('calendar.eventRun'),
        swim: t('calendar.eventSwim'),
        unknown: t('calendar.eventOther'),
      })
      : t('calendar.restDay'));
  const subtitle = raceLabel
    ? formatRaceSubtitle(raceLabel.payload, t)
    : (hasTraining
      ? buildSubtitle(visibleActivity, isPlannedOnly ? primaryPlannedWorkoutEvent : null, locale, {
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
      ? getTone(visibleActivity, primaryEvent)
      : 'muted';
  const bars = raceLabel
    ? buildRaceBars(raceLabel)
    : buildBars(visibleActivity, primaryPlannedWorkoutEvent);
  const Icon = raceLabel
    ? getRaceIcon(raceLabel.payload.discipline)
    : hasTraining
      ? getIcon(visibleActivity, primaryEvent)
      : BedDouble;
  const isSelectable = hasTraining && Boolean(onSelect);
  const plannedSyncVisual = isPredictedPlannedOnly ? getPlannedSyncVisual(plannedSyncStatus, t) : null;
  const racePriorityVisual = raceLabel ? getRacePriorityVisual(raceLabel.payload.priority) : null;
  const compactPrepTitle = hasCompactRacePrep && primaryPlannedWorkoutEvent
    ? buildCompactPlannedTitle(primaryPlannedWorkoutEvent, t)
    : null;
  const compactPrepSubtitle = hasCompactRacePrep && primaryPlannedWorkoutEvent
    ? buildCompactPlannedSubtitle(primaryPlannedWorkoutEvent, locale)
    : null;
  const compactPrepBars = hasCompactRacePrep && primaryPlannedWorkoutEvent
    ? buildCompactPlannedBars(primaryPlannedWorkoutEvent)
    : [];
  const matchedPlanBadgeLabel = hasMatchedPlannedWorkout ? t('calendar.planMatched') : null;

  const baseClassName = [
    'flex min-h-[160px] w-full flex-col gap-3 rounded-xl border p-3 text-left transition-colors md:min-h-[168px] md:p-3.5',
    hasTraining
      ? raceLabel
        ? 'bg-[linear-gradient(180deg,rgba(34,24,16,0.96),rgba(18,14,10,0.94))] border-[#cda56b]/30 shadow-[0_0_0_1px_rgba(205,165,107,0.08)]'
        : isPlannedRestDay
          ? 'bg-[#20181a] border-[#ff7351]/60 shadow-[0_0_0_1px_rgba(255,115,81,0.1)]'
        : isMissedPlannedOnly
          ? plannedSyncVisual
            ? `bg-[#191c1f] opacity-70 ${plannedSyncVisual.borderClass}`
            : 'bg-[#191c1f] border-white/8 opacity-70'
        : plannedSyncVisual
          ? `bg-[#1d2024] ${plannedSyncVisual.borderClass}`
          : 'bg-[#1d2024] border-white/5'
      : 'bg-[#1d2024]/85 border-white/5 opacity-60',
    isToday ? 'ring-1 ring-[#d2ff9a]/40 shadow-[0_0_0_1px_rgba(210,255,154,0.15)]' : '',
    isSelectable
      ? raceLabel
        ? 'cursor-pointer hover:border-[#e2ba7d]/45 hover:bg-[linear-gradient(180deg,rgba(39,28,19,0.98),rgba(22,17,13,0.96))]'
        : isPlannedRestDay
          ? 'cursor-default'
        : isMissedPlannedOnly && plannedSyncVisual
          ? `cursor-pointer hover:bg-[#20242a] ${plannedSyncVisual.hoverBorderClass}`
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
          {hasCompactRacePrep ? (
            <div className="mb-3 rounded-xl border border-[#00e3fd]/20 bg-[#0e1820]/80 px-3 py-2.5 shadow-[0_0_0_1px_rgba(0,227,253,0.04)]">
              <div className="flex items-start justify-between gap-3">
                <div className="min-w-0">
                  <p className="text-[9px] font-black uppercase tracking-[0.22em] text-[#00e3fd]">
                    {t('calendar.prepWorkout')}
                  </p>
                  <p className="truncate text-[11px] font-bold text-[#eafcff]">{compactPrepTitle}</p>
                </div>
                {compactPrepSubtitle ? (
                  <p className="shrink-0 text-[10px] font-semibold text-[#9fd8e3]">{compactPrepSubtitle}</p>
                ) : null}
              </div>
              {compactPrepBars.length > 0 ? (
                <div className="mt-2">
                  <CalendarMiniChart bars={compactPrepBars} tone="secondary" />
                </div>
              ) : null}
            </div>
          ) : null}
          <CalendarMiniChart bars={bars} tone={tone} />
          {raceLabel ? (
            <div className="mb-2 flex flex-wrap items-center gap-2">
              <p className="text-[10px] font-bold uppercase tracking-[0.18em] text-[#f2c98e]">
                {t('calendar.raceDay')}
              </p>
              <p className="text-[10px] font-semibold uppercase tracking-[0.18em] text-[#d7b37b]">
                {mapRaceDisciplineLabel(raceLabel.payload.discipline, t)}
              </p>
            </div>
          ) : hasMatchedPlannedWorkout ? (
            <div className="mb-2 flex flex-wrap gap-2">
              <p className="text-[10px] font-bold uppercase tracking-[0.18em] text-[#d2ff9a]">
                {t('calendar.completedWorkout')}
              </p>
              {matchedPlanBadgeLabel ? (
                <p className="text-[10px] font-bold uppercase tracking-[0.18em] text-[#00e3fd]">
                  {matchedPlanBadgeLabel}
                </p>
              ) : null}
            </div>
          ) : isPlannedRestDay ? (
            <div className="mb-2 flex flex-wrap gap-2">
              <p className="text-[10px] font-bold uppercase tracking-[0.18em] text-[#ff9b85]">
                {t('calendar.restDay')}
              </p>
            </div>
          ) : isPlannedOnly ? (
            <div className="mb-2 flex flex-wrap gap-2">
              <p className="text-[10px] font-bold uppercase tracking-[0.18em] text-[#00e3fd]">
                {t('calendar.plannedWorkout')}
              </p>
              {isMissedPlannedOnly ? (
                <p className="text-[10px] font-bold uppercase tracking-[0.18em] text-slate-400">
                  {t('calendar.notDone')}
                </p>
              ) : null}
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
          {isSelectable && interactiveDayItems.length > 0 && extraItemCount > 0 ? (
            <p className="mt-2 text-[10px] font-bold uppercase tracking-widest text-[#00e3fd]">
              {t('calendar.viewItems', { count: pickerVisibleItemCount })}
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
    if (dayEvent.restDay) {
      return dayEvent.name ?? 'Rest Day';
    }

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
  if (dayEvent?.restDay) {
    return dayEvent.restDayReason ?? 'Rest Day';
  }

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
  if (dayEvent?.restDay) {
    return 'error';
  }

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
  if (dayEvent?.restDay) {
    return BedDouble;
  }

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

function buildCompactPlannedBars(dayEvent: CalendarDayEvent): WorkoutBar[] {
  return downsampleWorkoutBars(buildPlannedWorkoutBars(dayEvent), 4)
    .map((bar) => ({
      ...bar,
      height: Math.max(26, Math.round(bar.height * 0.72)),
    }));
}

function buildBars(dayActivity: CalendarDay['activities'][number] | null, dayEvent: CalendarDay['events'][number] | null): Array<number | WorkoutBar> {
  if (dayEvent?.restDay) {
    return [18, 12, 20];
  }

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

function startOfDay(value: Date): Date {
  return new Date(value.getFullYear(), value.getMonth(), value.getDate());
}

function downsampleWorkoutBars(bars: WorkoutBar[], maxBars: number): WorkoutBar[] {
  if (bars.length <= maxBars) {
    return bars;
  }

  const totalWidthUnits = bars.reduce((sum, bar) => sum + Math.max(1, bar.widthUnits ?? 1), 0);
  if (totalWidthUnits <= 0) {
    return bars.slice(0, maxBars);
  }

  const targetBucketWidth = totalWidthUnits / maxBars;
  const groupedBars: WorkoutBar[] = [];
  let index = 0;

  for (let bucketIndex = 0; bucketIndex < maxBars && index < bars.length; bucketIndex += 1) {
    let consumedWidth = 0;
    let dominantBar = bars[index];
    let dominantHeight = dominantBar.height;
    let groupWidth = 0;

    while (index < bars.length && (consumedWidth < targetBucketWidth || groupWidth === 0)) {
      const currentBar = bars[index];
      const currentWidth = Math.max(1, currentBar.widthUnits ?? 1);
      consumedWidth += currentWidth;
      groupWidth += currentWidth;
      if (currentBar.height >= dominantHeight) {
        dominantBar = currentBar;
        dominantHeight = currentBar.height;
      }
      index += 1;
    }

    groupedBars.push({
      ...dominantBar,
      widthUnits: groupWidth,
    });
  }

  return groupedBars;
}

function buildCompactPlannedTitle(
  dayEvent: CalendarDayEvent,
  t: ReturnType<typeof useTranslation>['t'],
): string {
  return dayEvent.name?.trim() || t('calendar.plannedWorkout');
}

function buildCompactPlannedSubtitle(
  dayEvent: CalendarDayEvent,
  locale: string,
): string | null {
  const summary = dayEvent.eventDefinition.summary;
  const durationSeconds = summary.totalDurationSeconds;
  const estimatedTss = summary.estimatedTrainingStressScore;
  const durationMinutes = durationSeconds > 0
    ? new Intl.NumberFormat(locale, {
      style: 'unit',
      unit: 'minute',
      unitDisplay: 'short',
      maximumFractionDigits: 0,
    }).format(Math.round(durationSeconds / 60))
    : null;
  const tss = estimatedTss !== null && estimatedTss !== undefined
    ? Math.round(estimatedTss)
    : null;

  if (durationMinutes && tss !== null) {
    return `${durationMinutes} • ${tss} TSS`;
  }

  return durationMinutes ?? (tss !== null ? `${tss} TSS` : null);
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
