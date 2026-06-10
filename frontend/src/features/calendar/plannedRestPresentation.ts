import { formatPlannedRestRange } from '../planned-rest-days/utils';
import type { PlannedRestDay } from '../planned-rest-days/types';
import type { CalendarPlannedRestDayLabel } from './types';

export function plannedRestLabelToEntry(label: CalendarPlannedRestDayLabel): PlannedRestDay {
  return {
    plannedRestDayId: label.payload.plannedRestDayId,
    startDate: label.payload.startDate,
    endDate: label.payload.endDate,
    title: label.payload.title,
    note: label.payload.note,
    createdAtEpochSeconds: 0,
    updatedAtEpochSeconds: 0,
  };
}

export function formatPlannedRestLabelSubtitle(
  label: CalendarPlannedRestDayLabel,
  locale: string,
): string | null {
  const entry = plannedRestLabelToEntry(label);

  if (entry.startDate !== entry.endDate) {
    return formatPlannedRestRange(entry, locale);
  }

  return label.subtitle ?? entry.note;
}
