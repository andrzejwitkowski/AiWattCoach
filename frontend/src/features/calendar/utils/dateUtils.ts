export function startOfLocalDay(date: Date): Date {
  return new Date(date.getFullYear(), date.getMonth(), date.getDate());
}

export function parseDateKey(value: string): Date {
  const [year, month, day] = value.split('-').map(Number);
  return new Date(year, month - 1, day);
}

export function toDateKey(date: Date): string {
  const year = date.getFullYear();
  const month = String(date.getMonth() + 1).padStart(2, '0');
  const day = String(date.getDate()).padStart(2, '0');
  return `${year}-${month}-${day}`;
}

export function addDays(date: Date, days: number): Date {
  const next = new Date(date);
  next.setDate(next.getDate() + days);
  return startOfLocalDay(next);
}

export function addWeeks(date: Date, weeks: number): Date {
  return addDays(date, weeks * 7);
}

export function getMondayOfWeek(date: Date): Date {
  const normalized = startOfLocalDay(date);
  const day = normalized.getDay();
  const diff = day === 0 ? -6 : 1 - day;
  return addDays(normalized, diff);
}

export function generateWeekDates(mondayDate: Date): Date[] {
  return Array.from({ length: 7 }, (_, index) => addDays(mondayDate, index));
}

export function isSameDay(left: Date, right: Date): boolean {
  return toDateKey(left) === toDateKey(right);
}

export function isToday(date: Date): boolean {
  return isSameDay(date, new Date());
}

export function getWeekNumber(date: Date): number {
  const utcDate = new Date(Date.UTC(date.getFullYear(), date.getMonth(), date.getDate()));
  const dayNumber = utcDate.getUTCDay() === 0 ? 7 : utcDate.getUTCDay();
  utcDate.setUTCDate(utcDate.getUTCDate() + 4 - dayNumber);
  const yearStart = new Date(Date.UTC(utcDate.getUTCFullYear(), 0, 1));
  return Math.ceil((((utcDate.getTime() - yearStart.getTime()) / 86400000) + 1) / 7);
}

export function formatDayLabel(date: Date, locale: string): string {
  return new Intl.DateTimeFormat(locale, {
    month: 'short',
    day: '2-digit',
  }).format(date).toUpperCase();
}

export function formatDateRange(startMonday: Date, weeks: number): { oldest: string; newest: string } {
  const oldest = toDateKey(startMonday);
  const newest = toDateKey(addDays(startMonday, (weeks * 7) - 1));
  return { oldest, newest };
}

export function extractDateKey(value: string): string {
  return value.slice(0, 10);
}
