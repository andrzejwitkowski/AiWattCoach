import { useTranslation } from 'react-i18next';

import { CALENDAR_WEEK_ROW_HEIGHT } from '../constants';
import { buildDayItems, isInteractiveDayItem, type CalendarDayItemsSelection, selectDayItemDetail } from '../dayItems';
import type { CalendarRaceLabel, CalendarWeek } from '../types';
import { isToday } from '../utils/dateUtils';
import { isPlannedWorkoutEvent, selectWorkoutDetail, type WorkoutDetailSelection } from '../workoutDetails';
import { CalendarErrorRow } from './CalendarErrorRow';
import { CalendarDayCell } from './CalendarDayCell';
import { CalendarLoadingRow } from './CalendarLoadingRow';
import { CalendarWeekSummary } from './CalendarWeekSummary';

type CalendarWeekSectionProps = {
  week: CalendarWeek;
  showLoadingIndicator?: boolean;
  onSelectWorkout?: (selection: WorkoutDetailSelection) => void;
  onSelectDayItems?: (selection: CalendarDayItemsSelection) => void;
  onSelectRace?: (race: CalendarRaceLabel) => void;
};

export function CalendarWeekSection({
  week,
  showLoadingIndicator = true,
  onSelectWorkout,
  onSelectDayItems,
  onSelectRace,
}: CalendarWeekSectionProps) {
  const { t, i18n } = useTranslation();
  const locale = i18n.resolvedLanguage ?? i18n.language ?? 'en';

  if (week.status === 'loading' || week.status === 'idle') {
    return <CalendarLoadingRow status={week.status} showLoadingIndicator={showLoadingIndicator} />;
  }

  if (week.status === 'error') {
    return <CalendarErrorRow />;
  }

  return (
    <section
      className="flex flex-col gap-4 overflow-hidden"
      style={{ height: `${CALENDAR_WEEK_ROW_HEIGHT}px` }}
    >
      <CalendarWeekSummary weekNumber={week.weekNumber} summary={week.summary} />
      <div className="calendar-grid gap-3">
        {week.days.map((day) => {
          const dayItems = buildDayItems(day, {
            locale,
            labels: {
              plannedWorkout: t('calendar.plannedWorkout'),
              workout: t('calendar.workout'),
            },
            t,
          });
          const interactiveDayItems = dayItems.filter(isInteractiveDayItem);
          const hasInteractiveDayItem = interactiveDayItems.length > 0;
          const hasSelectableDayItem = hasInteractiveDayItem;
          const hasActualWorkoutEvent = day.events.some((event) => Boolean(event.actualWorkout));
          const canOpenPicker = dayItems.length > 1 && interactiveDayItems.length > 0 && Boolean(onSelectDayItems);
          const selectionHandler = (onSelectWorkout || onSelectDayItems || onSelectRace)
            && (hasSelectableDayItem || day.activities.length > 0 || hasActualWorkoutEvent || canOpenPicker)
            ? () => {
              if (dayItems.length > 1 && interactiveDayItems.length > 0 && onSelectDayItems) {
                onSelectDayItems?.({
                  dateKey: day.dateKey,
                  items: dayItems,
                });
                return;
              }

              const itemSelection = interactiveDayItems.length === 1 ? selectDayItemDetail(interactiveDayItems[0]) : null;
              if (interactiveDayItems.length === 1 && interactiveDayItems[0]?.kind === 'race' && onSelectRace) {
                onSelectRace?.(interactiveDayItems[0].race);
                return;
              }

              if (itemSelection) {
                onSelectWorkout?.(itemSelection);
                return;
              }

              if (onSelectWorkout) {
                const fallbackEvent = day.events.find((event) => event.actualWorkout || isPlannedWorkoutEvent(event)) ?? null;
                const fallbackSelection = selectWorkoutDetail(day.dateKey, fallbackEvent, day.activities);
                if (fallbackSelection.event || fallbackSelection.activity) {
                  onSelectWorkout(fallbackSelection);
                }
              }
            }
            : undefined;

          return (
          <CalendarDayCell
            key={day.dateKey}
            day={day}
            isToday={isToday(day.date)}
            onSelect={selectionHandler}
          />
          );
        })}
      </div>
    </section>
  );
}
