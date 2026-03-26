import { act, renderHook, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import type { IntervalActivity, IntervalEvent } from '../../intervals/types';
import { CALENDAR_BUFFER_WEEKS, CALENDAR_VISIBLE_WEEKS } from '../constants';
import { useCalendarData } from './useCalendarData';

vi.mock('../../intervals/api/intervals', () => ({
  listEvents: vi.fn(),
  listActivities: vi.fn(),
}));

import { listActivities, listEvents } from '../../intervals/api/intervals';

afterEach(() => {
  vi.clearAllMocks();
});

describe('useCalendarData', () => {
  it('keeps a fixed five-week window after initial load', async () => {
    vi.mocked(listEvents).mockResolvedValue([] satisfies IntervalEvent[]);
    vi.mocked(listActivities).mockResolvedValue([] satisfies IntervalActivity[]);

    const { result } = renderHook(() => useCalendarData({ apiBaseUrl: '' }));

    await waitFor(() => {
      expect(result.current.state).toBe('ready');
    });

    expect(result.current.weeks).toHaveLength(CALENDAR_VISIBLE_WEEKS);
  });

  it('keeps five rendered weeks after scrolling forward', async () => {
    vi.mocked(listEvents).mockResolvedValue([] satisfies IntervalEvent[]);
    vi.mocked(listActivities).mockResolvedValue([] satisfies IntervalActivity[]);

    const { result } = renderHook(() => useCalendarData({ apiBaseUrl: '' }));

    await waitFor(() => {
      expect(result.current.state).toBe('ready');
    });

    const initialFirstWeek = result.current.weeks[0]?.weekKey;

    await act(async () => {
      await result.current.loadMoreFuture();
    });

    await waitFor(() => {
      expect(result.current.weeks).toHaveLength(CALENDAR_VISIBLE_WEEKS);
      expect(result.current.weeks[0]?.weekKey).not.toBe(initialFirstWeek);
    });
  });

  it('refetches weeks that were pruned from the buffer when scrolling back', async () => {
    vi.mocked(listEvents).mockResolvedValue([] satisfies IntervalEvent[]);
    vi.mocked(listActivities).mockResolvedValue([] satisfies IntervalActivity[]);

    const { result } = renderHook(() => useCalendarData({ apiBaseUrl: '' }));

    await waitFor(() => {
      expect(result.current.state).toBe('ready');
    });

    for (let index = 0; index < CALENDAR_BUFFER_WEEKS + 1; index += 1) {
      await act(async () => {
        await result.current.loadMoreFuture();
      });
    }

    vi.clearAllMocks();

    await act(async () => {
      await result.current.loadMorePast();
    });

    await waitFor(() => {
      expect(listEvents).toHaveBeenCalled();
      expect(listActivities).toHaveBeenCalled();
    });
  });
});
