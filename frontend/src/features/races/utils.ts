import type { Race, RaceDiscipline, RacePriority } from './types';

export function parseRaceDate(date: string): Date {
  const [year, month, day] = date.split('-').map(Number);
  return new Date(year, month - 1, day);
}

export function toDateKey(date: Date): string {
  const year = date.getFullYear();
  const month = String(date.getMonth() + 1).padStart(2, '0');
  const day = String(date.getDate()).padStart(2, '0');
  return `${year}-${month}-${day}`;
}

export function formatRaceDate(date: string, locale: string): string {
  return new Intl.DateTimeFormat(locale, { day: '2-digit', month: 'short', year: 'numeric' }).format(parseRaceDate(date));
}

export function formatRaceDistance(distanceMeters: number, locale: string): string {
  return new Intl.NumberFormat(locale, {
    minimumFractionDigits: 0,
    maximumFractionDigits: 1,
  }).format(distanceMeters / 1000);
}

type Translate = (key: string, options?: Record<string, unknown>) => string;

export function mapRaceDisciplineLabel(discipline: RaceDiscipline, t: Translate): string {
  switch (discipline) {
    case 'road':
      return t('races.discipline.road');
    case 'mtb':
      return t('races.discipline.mtb');
    case 'gravel':
      return t('races.discipline.gravel');
    case 'cyclocross':
      return t('races.discipline.cyclocross');
    case 'timetrial':
      return t('races.discipline.timetrial');
  }
}

export function sortRacesAscending(left: Race, right: Race): number {
  return left.date.localeCompare(right.date) || left.name.localeCompare(right.name);
}

export function sortRacesDescending(left: Race, right: Race): number {
  return right.date.localeCompare(left.date) || left.name.localeCompare(right.name);
}

export function splitRacesByDate(races: Race[], todayDateKey: string): { upcoming: Race[]; completed: Race[] } {
  const upcoming = races.filter((race) => race.date >= todayDateKey).sort(sortRacesAscending);
  const completed = races.filter((race) => race.date < todayDateKey).sort(sortRacesDescending);

  return { upcoming, completed };
}

export function getPriorityTone(priority: RacePriority): string {
  switch (priority) {
    case 'A':
      return 'border-[#e9c98b]/35 bg-[#e9c98b]/12 text-[#f6deb1]';
    case 'B':
      return 'border-[#b6b0a6]/30 bg-[#b6b0a6]/10 text-[#d8d3cb]';
    case 'C':
    default:
      return 'border-[#9c6840]/30 bg-[#9c6840]/10 text-[#d7b08d]';
  }
}
