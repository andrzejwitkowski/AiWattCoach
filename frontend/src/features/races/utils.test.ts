import { describe, expect, it } from 'vitest';

import type { Race } from './types';
import { splitRacesByDate } from './utils';

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
});
