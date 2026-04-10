import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import '../../../i18n';
import { CALENDAR_PAGINATION_TRIGGER_OFFSET } from '../constants';
import type { CalendarWeek } from '../types';
import { CalendarGrid } from './CalendarGrid';

vi.mock('../hooks/useCalendarData', () => ({
  useCalendarData: vi.fn(),
}));

import { useCalendarData } from '../hooks/useCalendarData';

afterEach(() => {
  cleanup();
  vi.mocked(useCalendarData).mockReset();
});

function buildWeek(weekKey: string): CalendarWeek {
  const mondayDate = new Date(`${weekKey}T00:00:00`);

  return {
    weekNumber: 12,
    weekKey,
    mondayDate,
    status: 'loaded',
    summary: {
      totalTss: 0,
      targetTss: null,
      totalCalories: 0,
      totalDurationSeconds: 0,
      targetDurationSeconds: null,
      totalDistanceMeters: 0,
    },
    days: Array.from({ length: 7 }, (_, index) => {
      const date = new Date(mondayDate);
      date.setDate(date.getDate() + index);
      return {
        date,
        dateKey: date.toISOString().slice(0, 10),
        events: [],
        activities: [],
        labels: [],
      };
    }),
  };
}

function buildRenderedWeeks(): CalendarWeek[] {
  return [
    buildWeek('2026-02-16'),
    buildWeek('2026-02-23'),
    buildWeek('2026-03-02'),
    buildWeek('2026-03-09'),
    buildWeek('2026-03-16'),
    buildWeek('2026-03-23'),
    buildWeek('2026-03-30'),
    buildWeek('2026-04-06'),
    buildWeek('2026-04-13'),
    buildWeek('2026-04-20'),
    buildWeek('2026-04-27'),
  ];
}

function buildHookState(overrides: Partial<ReturnType<typeof useCalendarData>> = {}): ReturnType<typeof useCalendarData> {
  return {
    state: 'ready',
    weeks: [
      buildWeek('2026-03-23'),
      buildWeek('2026-03-30'),
      buildWeek('2026-04-06'),
      buildWeek('2026-04-13'),
      buildWeek('2026-04-20'),
    ],
    renderedWeeks: buildRenderedWeeks(),
    topPreviewWeek: buildWeek('2026-03-16'),
    bottomPreviewWeek: buildWeek('2026-04-27'),
    isLoadingPast: false,
    isLoadingFuture: false,
    scrollAdjustment: { topDelta: 0, version: 0 },
    loadMorePast: vi.fn(),
    loadMoreFuture: vi.fn(),
    ...overrides,
  };
}

describe('CalendarGrid', () => {
  it('only triggers past pagination once until the user leaves the top edge', () => {
    const loadMorePast = vi.fn();
    vi.mocked(useCalendarData).mockReturnValue(buildHookState({ loadMorePast }));

    render(<CalendarGrid apiBaseUrl="" />);

    const scroller = screen.getByRole('region', { name: /performance calendar/i });
    Object.defineProperty(scroller, 'clientHeight', { configurable: true, value: 600 });
    Object.defineProperty(scroller, 'scrollHeight', { configurable: true, value: 2400 });
    Object.defineProperty(scroller, 'scrollTop', { configurable: true, writable: true, value: CALENDAR_PAGINATION_TRIGGER_OFFSET - 1 });

    fireEvent.scroll(scroller);
    fireEvent.scroll(scroller);

    expect(loadMorePast).toHaveBeenCalledTimes(1);

    scroller.scrollTop = CALENDAR_PAGINATION_TRIGGER_OFFSET + 100;
    fireEvent.scroll(scroller);

    scroller.scrollTop = CALENDAR_PAGINATION_TRIGGER_OFFSET - 2;
    fireEvent.scroll(scroller);

    expect(loadMorePast).toHaveBeenCalledTimes(2);
  });

  it('does not trigger a past load while future pagination is in flight', () => {
    const loadMorePast = vi.fn();
    const loadMoreFuture = vi.fn();
    vi.mocked(useCalendarData).mockReturnValue(buildHookState({ isLoadingFuture: true, loadMorePast, loadMoreFuture }));

    render(<CalendarGrid apiBaseUrl="" />);

    const scroller = screen.getByRole('region', { name: /performance calendar/i });
    Object.defineProperty(scroller, 'scrollTop', { configurable: true, value: 0 });
    Object.defineProperty(scroller, 'clientHeight', { configurable: true, value: 200 });
    Object.defineProperty(scroller, 'scrollHeight', { configurable: true, value: 1000 });

    fireEvent.scroll(scroller);

    expect(loadMorePast).not.toHaveBeenCalled();
  });

  it('does not trigger a future load while past pagination is in flight', () => {
    const loadMorePast = vi.fn();
    const loadMoreFuture = vi.fn();
    vi.mocked(useCalendarData).mockReturnValue(buildHookState({ isLoadingPast: true, loadMorePast, loadMoreFuture }));

    render(<CalendarGrid apiBaseUrl="" />);

    const scroller = screen.getByRole('region', { name: /performance calendar/i });
    Object.defineProperty(scroller, 'scrollTop', { configurable: true, value: 900 });
    Object.defineProperty(scroller, 'clientHeight', { configurable: true, value: 200 });
    Object.defineProperty(scroller, 'scrollHeight', { configurable: true, value: 1000 });

    fireEvent.scroll(scroller);

    expect(loadMoreFuture).not.toHaveBeenCalled();
  });

  it('waits for more downward travel before triggering another future load after a shift', () => {
    const loadMoreFuture = vi.fn();
    const hookState = buildHookState({ loadMoreFuture });

    vi.mocked(useCalendarData).mockImplementation(() => hookState);

    const { rerender } = render(<CalendarGrid apiBaseUrl="" />);

    const scroller = screen.getByRole('region', { name: /performance calendar/i });
    Object.defineProperty(scroller, 'clientHeight', { configurable: true, value: 200 });
    Object.defineProperty(scroller, 'scrollHeight', { configurable: true, value: 1000 });
    Object.defineProperty(scroller, 'scrollTop', { configurable: true, writable: true, value: 790 });

    fireEvent.scroll(scroller);

    expect(loadMoreFuture).toHaveBeenCalledTimes(1);

    hookState.scrollAdjustment = { topDelta: -360, version: 1 };
    rerender(<CalendarGrid apiBaseUrl="" />);

    scroller.scrollTop = 480;
    fireEvent.scroll(scroller);
    expect(loadMoreFuture).toHaveBeenCalledTimes(1);

    scroller.scrollTop = 520;
    fireEvent.scroll(scroller);
    expect(loadMoreFuture).toHaveBeenCalledTimes(1);

    scroller.scrollTop = 790;
    fireEvent.scroll(scroller);
    expect(loadMoreFuture).toHaveBeenCalledTimes(2);
  });

  it('keeps loading feedback inside the preview and visible weeks only', () => {
    vi.mocked(useCalendarData).mockReturnValue(buildHookState({ isLoadingPast: true }));

    render(<CalendarGrid apiBaseUrl="" />);

    expect(screen.queryByText(/fetching data/i)).not.toBeInTheDocument();
  });
});
