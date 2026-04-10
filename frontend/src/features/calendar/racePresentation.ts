import type { CalendarRaceLabel } from './types';

type Translate = (key: string, options?: Record<string, unknown>) => string;

export function formatRaceSubtitle(race: CalendarRaceLabel['payload'], t: Translate): string {
  return `${Math.round(race.distanceMeters / 1000)} km • ${t('calendar.priorityLabel', { priority: race.priority })}`;
}

export function mapRaceDisciplineLabel(
  discipline: CalendarRaceLabel['payload']['discipline'],
  t: Translate,
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
