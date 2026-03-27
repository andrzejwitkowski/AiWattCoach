import { act, renderHook, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import type { IntervalActivity, IntervalEvent } from '../../intervals/types';
import { AuthenticationError, HttpError } from '../../../lib/httpClient';
import { CALENDAR_BUFFER_WEEKS, CALENDAR_VISIBLE_WEEKS } from '../constants';
import { addDays, parseDateKey, toDateKey } from '../utils/dateUtils';
import { useCalendarData } from './useCalendarData';

vi.mock('../../intervals/api/intervals', () => ({
  listEvents: vi.fn(),
  listActivities: vi.fn(),
}));

import { listActivities, listEvents } from '../../intervals/api/intervals';

const originalLocation = window.location;

function createDeferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((onResolve) => {
    resolve = onResolve;
  });

  return { promise, resolve };
}

function hasRangeCall(mock: ReturnType<typeof vi.fn>, oldest: string, newest: string): boolean {
  return countRangeCalls(mock, oldest, newest) > 0;
}

function countRangeCalls(mock: ReturnType<typeof vi.fn>, oldest: string, newest: string): number {
  return mock.mock.calls.filter(([, query]) => {
    return query !== null
      && typeof query === 'object'
      && 'oldest' in query
      && 'newest' in query
      && query.oldest === oldest
      && query.newest === newest;
  }).length;
}

afterEach(() => {
  vi.clearAllMocks();
  Object.defineProperty(window, 'location', {
    configurable: true,
    value: originalLocation,
  });
});

describe('useCalendarData', () => {
  it('defaults unresolved weeks to idle placeholders', () => {
    const deferredEvents = createDeferred<IntervalEvent[]>();
    const deferredActivities = createDeferred<IntervalActivity[]>();
    vi.mocked(listEvents).mockReturnValue(deferredEvents.promise);
    vi.mocked(listActivities).mockReturnValue(deferredActivities.promise);

    const { result, unmount } = renderHook(() => useCalendarData({ apiBaseUrl: '' }));

    expect(result.current.weeks.every((week) => week.status === 'idle')).toBe(true);
    expect(result.current.topPreviewWeek.status).toBe('idle');
    expect(result.current.bottomPreviewWeek.status).toBe('idle');

    unmount();
    deferredEvents.resolve([]);
    deferredActivities.resolve([]);
  });

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

    const initialFirstWeek = result.current.weeks[0]!.weekKey;
    const initialLastDay = toDateKey(addDays(parseDateKey(initialFirstWeek), 6));

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
      expect(hasRangeCall(vi.mocked(listEvents), initialFirstWeek, initialLastDay)).toBe(true);
      expect(hasRangeCall(vi.mocked(listActivities), initialFirstWeek, initialLastDay)).toBe(true);
    });
  });

  it('coalesces concurrent forward loads into a single request', async () => {
    vi.mocked(listEvents).mockResolvedValue([] satisfies IntervalEvent[]);
    vi.mocked(listActivities).mockResolvedValue([] satisfies IntervalActivity[]);

    const { result } = renderHook(() => useCalendarData({ apiBaseUrl: '' }));

    await waitFor(() => {
      expect(result.current.state).toBe('ready');
    });

    const deferredEvents = createDeferred<IntervalEvent[]>();
    const deferredActivities = createDeferred<IntervalActivity[]>();

    vi.clearAllMocks();
    vi.mocked(listEvents).mockReturnValueOnce(deferredEvents.promise);
    vi.mocked(listActivities).mockReturnValueOnce(deferredActivities.promise);

    let firstLoad!: Promise<void>;
    let secondLoad!: Promise<void>;

    await act(async () => {
      firstLoad = result.current.loadMoreFuture();
      secondLoad = result.current.loadMoreFuture();
      await Promise.resolve();
    });

    expect(listEvents).toHaveBeenCalledTimes(1);
    expect(listActivities).toHaveBeenCalledTimes(1);

    deferredEvents.resolve([]);
    deferredActivities.resolve([]);

    await act(async () => {
      await Promise.all([firstLoad, secondLoad]);
    });
  });

  it('blocks an opposite-direction load while pagination is in flight', async () => {
    vi.mocked(listEvents).mockResolvedValue([] satisfies IntervalEvent[]);
    vi.mocked(listActivities).mockResolvedValue([] satisfies IntervalActivity[]);

    const { result } = renderHook(() => useCalendarData({ apiBaseUrl: '' }));

    await waitFor(() => {
      expect(result.current.state).toBe('ready');
    });

    for (let index = 0; index < CALENDAR_BUFFER_WEEKS; index += 1) {
      await act(async () => {
        await result.current.loadMoreFuture();
      });
    }

    const expectedFirstWeekAfterForward = result.current.weeks[1]!.weekKey;
    const deferredEvents = createDeferred<IntervalEvent[]>();
    const deferredActivities = createDeferred<IntervalActivity[]>();

    vi.clearAllMocks();
    vi.mocked(listEvents).mockReturnValueOnce(deferredEvents.promise);
    vi.mocked(listActivities).mockReturnValueOnce(deferredActivities.promise);

    let forwardLoad!: Promise<void>;
    let backwardLoad!: Promise<void>;

    await act(async () => {
      forwardLoad = result.current.loadMoreFuture();
      backwardLoad = result.current.loadMorePast();
      await Promise.resolve();
    });

    expect(listEvents).toHaveBeenCalledTimes(1);
    expect(listActivities).toHaveBeenCalledTimes(1);
    expect(result.current.weeks[0]!.weekKey).toBe(expectedFirstWeekAfterForward);

    deferredEvents.resolve([]);
    deferredActivities.resolve([]);

    await act(async () => {
      await Promise.all([forwardLoad, backwardLoad]);
    });
  });

  it('redirects to the landing page when calendar requests return unauthorized', async () => {
    vi.mocked(listEvents).mockRejectedValue(new AuthenticationError());
    vi.mocked(listActivities).mockResolvedValue([] satisfies IntervalActivity[]);

    Object.defineProperty(window, 'location', {
      configurable: true,
      value: { ...window.location, href: '/calendar' },
    });

    renderHook(() => useCalendarData({ apiBaseUrl: '' }));

    await waitFor(() => {
      expect(window.location.href).toBe('/');
    });
  });

  it('retries the same future range after a gateway failure on the next attempt', async () => {
    vi.mocked(listEvents).mockResolvedValue([] satisfies IntervalEvent[]);
    vi.mocked(listActivities).mockResolvedValue([] satisfies IntervalActivity[]);

    const { result } = renderHook(() => useCalendarData({ apiBaseUrl: '' }));

    await waitFor(() => {
      expect(result.current.state).toBe('ready');
    });

    const repeatedFailureWeek = toDateKey(
      addDays(result.current.bottomPreviewWeek.mondayDate, CALENDAR_BUFFER_WEEKS * 7)
    );
    const repeatedFailureWeekEnd = toDateKey(addDays(parseDateKey(repeatedFailureWeek), 6));

    vi.clearAllMocks();
    vi.mocked(listEvents).mockRejectedValue(new HttpError(502, 'bad gateway'));
    vi.mocked(listActivities).mockRejectedValue(new HttpError(502, 'bad gateway'));

    for (let attempt = 0; attempt < CALENDAR_BUFFER_WEEKS + 2; attempt += 1) {
      await act(async () => {
        await result.current.loadMoreFuture();
      });
    }

    expect(countRangeCalls(vi.mocked(listEvents), repeatedFailureWeek, repeatedFailureWeekEnd)).toBeGreaterThan(1);
    expect(countRangeCalls(vi.mocked(listActivities), repeatedFailureWeek, repeatedFailureWeekEnd)).toBeGreaterThan(1);
  });

});
