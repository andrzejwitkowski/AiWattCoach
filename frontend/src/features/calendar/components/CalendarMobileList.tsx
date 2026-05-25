import { ChevronLeft, ChevronRight } from 'lucide-react';
import { useMemo } from 'react';
import { useTranslation } from 'react-i18next';

import { buildDayItems, isInteractiveDayItem, type CalendarDayItem, type CalendarDayItemsSelection, selectDayItemDetail } from '../dayItems';
import { formatRaceSubtitle } from '../racePresentation';
import type { CalendarRaceLabel, CalendarWeek } from '../types';
import { formatDayLabel, toDateKey } from '../utils/dateUtils';
import { isPlannedWorkoutEvent, type WorkoutDetailSelection } from '../workoutDetails';

type CalendarMobileListProps = {
  weeks: CalendarWeek[];
  locale: string;
  isLoadingPast: boolean;
  isLoadingFuture: boolean;
  onLoadMorePast: () => void;
  onLoadMoreFuture: () => void;
  onSelectWorkout: (selection: WorkoutDetailSelection) => void;
  onSelectDayItems: (selection: CalendarDayItemsSelection) => void;
  onSelectRace: (race: CalendarRaceLabel) => void;
};

export function CalendarMobileList({
  weeks,
  locale,
  isLoadingPast,
  isLoadingFuture,
  onLoadMorePast,
  onLoadMoreFuture,
  onSelectWorkout,
  onSelectDayItems,
  onSelectRace,
}: CalendarMobileListProps) {
  const { t } = useTranslation();
  const todayKey = useMemo(() => toDateKey(new Date()), []);

  return (
    <div className="space-y-4 md:hidden">
      <div className="flex items-center justify-between gap-3 rounded-2xl border border-white/8 bg-[#11161f] px-3 py-3">
        <button
          type="button"
          className="flex h-10 w-10 items-center justify-center rounded-xl border border-white/10 bg-white/5 text-slate-200 transition hover:bg-white/10 disabled:opacity-50"
          onClick={onLoadMorePast}
          disabled={isLoadingPast || isLoadingFuture}
          aria-label={t('calendar.mobilePreviousWeeks')}
        >
          <ChevronLeft size={18} />
        </button>
        <div className="text-center">
          <p className="text-[10px] font-black uppercase tracking-[0.24em] text-slate-500">{t('calendar.visibleWindow')}</p>
          <p className="mt-1 text-sm font-semibold text-white">{t('calendar.fiveWeeks')}</p>
        </div>
        <button
          type="button"
          className="flex h-10 w-10 items-center justify-center rounded-xl border border-white/10 bg-white/5 text-slate-200 transition hover:bg-white/10 disabled:opacity-50"
          onClick={onLoadMoreFuture}
          disabled={isLoadingFuture || isLoadingPast}
          aria-label={t('calendar.mobileNextWeeks')}
        >
          <ChevronRight size={18} />
        </button>
      </div>

      {weeks.map((week) => (
        <section key={week.weekKey} className="space-y-3 rounded-[1.5rem] border border-white/8 bg-[#10151d] p-3 shadow-[0_18px_40px_rgba(0,0,0,0.25)]">
          <div className="flex items-center justify-between gap-3 border-b border-white/6 pb-3">
            <div>
              <p className="text-[10px] font-black uppercase tracking-[0.24em] text-slate-500">{t('calendar.week')}</p>
              <h3 className="mt-1 text-lg font-black text-white">{week.weekNumber}</h3>
            </div>
            <div className="text-right text-xs text-slate-400">
              <p>{t('calendar.duration')}: {formatDurationMinutes(week.summary.totalDurationSeconds)}</p>
              <p>{t('calendar.energy')}: {week.summary.totalCalories}</p>
            </div>
          </div>

          <div className="space-y-2.5">
            {week.days.map((day) => {
              const dayItems = buildDayItems(day, {
                locale,
                labels: {
                  plannedWorkout: t('calendar.plannedWorkout'),
                  workout: t('calendar.workout'),
                },
                t,
              });
              const interactiveItems = dayItems.filter(isInteractiveDayItem);
              const primaryItem = interactiveItems[0] ?? null;
              const today = day.dateKey === todayKey;
              const count = dayItems.length;
              const title = primaryItem?.title ?? t('calendar.restDay');
              const subtitle = primaryItem?.subtitle ?? describeDayFallback(day, t);
              const label = primaryItem ? itemKindLabel(primaryItem, t) : t('calendar.restDay');

              return (
                <button
                  key={day.dateKey}
                  type="button"
                  onClick={() => selectDay(dayItems, interactiveItems, onSelectWorkout, onSelectDayItems, onSelectRace)}
                  disabled={interactiveItems.length === 0}
                  className={[
                    'w-full rounded-[1.25rem] border px-4 py-4 text-left transition',
                    interactiveItems.length > 0
                      ? 'border-white/8 bg-white/[0.03] hover:border-cyan-300/25 hover:bg-white/[0.05]'
                      : 'border-white/6 bg-white/[0.02] opacity-80',
                    today ? 'ring-1 ring-[#d2ff9a]/35' : '',
                  ].join(' ')}
                >
                  <div className="flex items-start justify-between gap-3">
                    <div className="min-w-0">
                      <p className="text-[10px] font-black uppercase tracking-[0.22em] text-slate-500">
                        {formatDayLabel(day.date, locale)}
                      </p>
                      <p className="mt-2 truncate text-base font-bold text-white">{title}</p>
                      <p className="mt-1 text-sm text-slate-400">{subtitle}</p>
                    </div>
                    <div className="shrink-0 text-right">
                      <p className="text-[10px] font-bold uppercase tracking-[0.2em] text-cyan-200">{label}</p>
                      {count > 1 ? (
                        <p className="mt-2 text-[11px] font-semibold text-slate-400">{t('calendar.mobileItemsCount', { count })}</p>
                      ) : null}
                    </div>
                  </div>
                </button>
              );
            })}
          </div>
        </section>
      ))}
    </div>
  );
}

function selectDay(
  dayItems: CalendarDayItem[],
  interactiveItems: CalendarDayItem[],
  onSelectWorkout: (selection: WorkoutDetailSelection) => void,
  onSelectDayItems: (selection: CalendarDayItemsSelection) => void,
  onSelectRace: (race: CalendarRaceLabel) => void,
) {
  if (interactiveItems.length === 0) {
    return;
  }

  if (dayItems.length > 1) {
    onSelectDayItems({
      dateKey: dayItems[0]?.dateKey ?? '',
      items: dayItems,
    });
    return;
  }

  const item = interactiveItems[0];
  if (!item) {
    return;
  }

  if (item.kind === 'race') {
    onSelectRace(item.race);
    return;
  }

  const detail = selectDayItemDetail(item);
  if (detail) {
    onSelectWorkout(detail);
  }
}

function itemKindLabel(item: CalendarDayItem, t: ReturnType<typeof useTranslation>['t']) {
  switch (item.kind) {
    case 'planned':
      return t('calendar.plannedWorkout');
    case 'completed':
      return t('calendar.completedWorkout');
    case 'race':
      return t('calendar.raceDay');
    case 'event':
    default:
      return t('calendar.eventOther');
  }
}

function describeDayFallback(
  day: CalendarWeek['days'][number],
  t: ReturnType<typeof useTranslation>['t'],
) {
  const race = day.labels.find((label): label is CalendarRaceLabel => label.kind === 'race');
  if (race) {
    return formatRaceSubtitle(race.payload, t);
  }

  const plannedEvent = day.events.find((event) => isPlannedWorkoutEvent(event));
  if (plannedEvent) {
    return plannedEvent.restDay ? t('calendar.restDay') : t('calendar.plannedWorkout');
  }

  if (day.activities.length > 0) {
    return t('calendar.completedWorkout');
  }

  return t('calendar.restDay');
}

function formatDurationMinutes(durationSeconds: number): string {
  if (durationSeconds <= 0) {
    return '0m';
  }

  const totalMinutes = Math.round(durationSeconds / 60);
  if (totalMinutes < 60) {
    return `${totalMinutes}m`;
  }

  const hours = Math.floor(totalMinutes / 60);
  const minutes = totalMinutes % 60;
  return minutes > 0 ? `${hours}h ${minutes}m` : `${hours}h`;
}
