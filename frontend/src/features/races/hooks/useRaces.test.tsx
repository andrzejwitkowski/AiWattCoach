import { renderHook, act } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import type { Race } from '../types';
import { useRaces } from './useRaces';

vi.mock('../api/races', () => ({
  listRaces: vi.fn(),
}));

import { listRaces } from '../api/races';

const raceFixture: Race = {
  raceId: 'race-1',
  date: '2026-04-10',
  name: 'Sunset Circuit',
  distanceMeters: 45000,
  discipline: 'road',
  priority: 'A',
  syncStatus: 'synced',
  linkedIntervalsEventId: null,
  lastError: null,
};

afterEach(() => {
  vi.useRealTimers();
  vi.clearAllMocks();
});

describe('useRaces', () => {
  it('reclassifies races after midnight without a refresh', async () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date('2026-04-10T23:59:59.000'));
    vi.mocked(listRaces).mockResolvedValue([raceFixture]);

    const { result } = renderHook(() => useRaces({ apiBaseUrl: '' }));

    await act(async () => {
      await vi.advanceTimersByTimeAsync(0);
    });

    expect(result.current.isLoading).toBe(false);
    expect(result.current.upcomingRaces).toEqual([raceFixture]);
    expect(result.current.completedRaces).toEqual([]);
    expect(listRaces).toHaveBeenCalledTimes(1);

    await act(async () => {
      vi.setSystemTime(new Date('2026-04-11T00:00:01.000'));
      await vi.advanceTimersByTimeAsync(2_000);
    });

    expect(result.current.upcomingRaces).toEqual([]);
    expect(result.current.completedRaces).toEqual([raceFixture]);

    expect(listRaces).toHaveBeenCalledTimes(1);
  });
});
