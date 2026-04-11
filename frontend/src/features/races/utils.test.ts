import { describe, expect, it } from 'vitest';

import en from '../../locales/en/translation.json';
import pl from '../../locales/pl/translation.json';
import type { Race } from './types';
import { formatRaceDistance, mapRaceDisciplineLabel, splitRacesByDate } from './utils';

function translate(messages: Record<string, unknown>) {
  return (key: string): string => {
    const value = key.split('.').reduce<unknown>((current, segment) => {
      if (current && typeof current === 'object' && segment in current) {
        return (current as Record<string, unknown>)[segment];
      }

      return undefined;
    }, messages);

    if (typeof value !== 'string') {
      throw new Error(`Missing translation for ${key}`);
    }

    return value;
  };
}

function makeRace(overrides: Partial<Race> = {}): Race {
  return {
    raceId: 'race-1',
    date: '2026-09-12',
    name: 'Race One',
    distanceMeters: 120000,
    discipline: 'road',
    priority: 'A',
    syncStatus: 'pending',
    linkedIntervalsEventId: null,
    lastError: null,
    ...overrides,
  };
}

describe('race utils', () => {
  it('splits races into upcoming and completed groups', () => {
    const races = [
      makeRace({ raceId: 'past', date: '2026-08-01', name: 'Past Race' }),
      makeRace({ raceId: 'future-b', date: '2026-09-20', name: 'B Race' }),
      makeRace({ raceId: 'future-a', date: '2026-09-12', name: 'A Race' }),
    ];

    const result = splitRacesByDate(races, '2026-09-12');

    expect(result.upcoming.map((race) => race.raceId)).toEqual(['future-a', 'future-b']);
    expect(result.completed.map((race) => race.raceId)).toEqual(['past']);
  });

  it('maps race discipline labels through races translations', () => {
    const tEn = translate(en);
    const tPl = translate(pl);

    expect(mapRaceDisciplineLabel('road', tEn)).toBe('Road');
    expect(mapRaceDisciplineLabel('road', tPl)).toBe('Szosa');
    expect(mapRaceDisciplineLabel('cyclocross', tPl)).toBe('Przełaj');
  });

  it('preserves fractional kilometer distance formatting', () => {
    expect(formatRaceDistance(42500, 'en')).toBe('42.5');
    expect(formatRaceDistance(42000, 'en')).toBe('42');
  });
});
