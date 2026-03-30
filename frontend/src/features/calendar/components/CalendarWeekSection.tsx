import { CALENDAR_WEEK_ROW_HEIGHT } from '../constants';
import type { CalendarWeek } from '../types';
import { isToday } from '../utils/dateUtils';
import { selectWorkoutDetail, type WorkoutDetailSelection } from '../workoutDetails';
import { CalendarErrorRow } from './CalendarErrorRow';
import { CalendarDayCell } from './CalendarDayCell';
import { CalendarLoadingRow } from './CalendarLoadingRow';
import { CalendarWeekSummary } from './CalendarWeekSummary';

type CalendarWeekSectionProps = {
  week: CalendarWeek;
  showLoadingIndicator?: boolean;
  onSelectWorkout?: (selection: WorkoutDetailSelection) => void;
};

export function CalendarWeekSection({ week, showLoadingIndicator = true, onSelectWorkout }: CalendarWeekSectionProps) {
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
          const selectionHandler = onSelectWorkout
            ? () => onSelectWorkout(selectWorkoutDetail(day.dateKey, day.events[0] ?? null, day.activities))
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
