import { CALENDAR_WEEK_ROW_HEIGHT } from '../constants';
import type { CalendarWeek } from '../types';
import { isToday } from '../utils/dateUtils';
import { CalendarErrorRow } from './CalendarErrorRow';
import { CalendarDayCell } from './CalendarDayCell';
import { CalendarLoadingRow } from './CalendarLoadingRow';
import { CalendarWeekSummary } from './CalendarWeekSummary';

type CalendarWeekSectionProps = {
  week: CalendarWeek;
};

export function CalendarWeekSection({ week }: CalendarWeekSectionProps) {
  if (week.status === 'loading' || week.status === 'idle') {
    return <CalendarLoadingRow />;
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
        {week.days.map((day) => (
          <CalendarDayCell key={day.dateKey} day={day} isToday={isToday(day.date)} />
        ))}
      </div>
    </section>
  );
}
