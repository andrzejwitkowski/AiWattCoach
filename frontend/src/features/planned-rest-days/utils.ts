import type { PlannedRestDay } from './types';

export function parsePlannedRestDate(date: string): Date {
  const [year, month, day] = date.split('-').map(Number);
  return new Date(year, month - 1, day);
}

export function toDateKey(date: Date): string {
  const year = date.getFullYear();
  const month = String(date.getMonth() + 1).padStart(2, '0');
  const day = String(date.getDate()).padStart(2, '0');
  return `${year}-${month}-${day}`;
}

export function formatPlannedRestDate(date: string, locale: string): string {
  return new Intl.DateTimeFormat(locale, { day: '2-digit', month: 'short', year: 'numeric' }).format(
    parsePlannedRestDate(date),
  );
}

export function formatPlannedRestRange(entry: PlannedRestDay, locale: string): string {
  if (entry.startDate === entry.endDate) {
    return formatPlannedRestDate(entry.startDate, locale);
  }

  return `${formatPlannedRestDate(entry.startDate, locale)} – ${formatPlannedRestDate(entry.endDate, locale)}`;
}

export function countPlannedRestDays(entry: PlannedRestDay): number {
  const start = parsePlannedRestDate(entry.startDate);
  const end = parsePlannedRestDate(entry.endDate);
  const diffMs = end.getTime() - start.getTime();
  return Math.floor(diffMs / (24 * 60 * 60 * 1000)) + 1;
}

export function countUniquePlannedRestCalendarDays(entries: PlannedRestDay[]): number {
  const dates = new Set<string>();

  for (const entry of entries) {
    const start = parsePlannedRestDate(entry.startDate);
    const end = parsePlannedRestDate(entry.endDate);
    const cursor = new Date(start);

    while (cursor <= end) {
      dates.add(toDateKey(cursor));
      cursor.setDate(cursor.getDate() + 1);
    }
  }

  return dates.size;
}

export function sortPlannedRestAscending(left: PlannedRestDay, right: PlannedRestDay): number {
  return (
    left.startDate.localeCompare(right.startDate)
    || left.endDate.localeCompare(right.endDate)
    || (left.title ?? '').localeCompare(right.title ?? '')
  );
}

export function sortPlannedRestDescending(left: PlannedRestDay, right: PlannedRestDay): number {
  return (
    right.endDate.localeCompare(left.endDate)
    || right.startDate.localeCompare(left.startDate)
    || (left.title ?? '').localeCompare(right.title ?? '')
  );
}

export function splitPlannedRestDaysByDate(
  entries: PlannedRestDay[],
  todayDateKey: string,
): { upcoming: PlannedRestDay[]; past: PlannedRestDay[] } {
  const upcoming = entries.filter((entry) => entry.endDate >= todayDateKey).sort(sortPlannedRestAscending);
  const past = entries.filter((entry) => entry.endDate < todayDateKey).sort(sortPlannedRestDescending);
  return { upcoming, past };
}
