import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import '../../../i18n';
import type { CalendarWeek } from '../types';
import { CalendarGrid } from './CalendarGrid';

vi.mock('../hooks/useCalendarData', () => ({
  useCalendarData: vi.fn(),
}));

import { useCalendarData } from '../hooks/useCalendarData';

afterEach(() => {
  cleanup();
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
      };
    }),
  };
}

describe('CalendarGrid', () => {
  it('does not trigger a past load while future pagination is in flight', () => {
    const loadMorePast = vi.fn();
    const loadMoreFuture = vi.fn();
    vi.mocked(useCalendarData).mockReturnValue({
      state: 'ready',
      weeks: [
        buildWeek('2026-03-23'),
        buildWeek('2026-03-30'),
        buildWeek('2026-04-06'),
        buildWeek('2026-04-13'),
        buildWeek('2026-04-20'),
      ],
      topPreviewWeek: buildWeek('2026-03-16'),
      bottomPreviewWeek: buildWeek('2026-04-27'),
      isLoadingPast: false,
      isLoadingFuture: true,
      loadingEdge: null,
      scrollAdjustment: { topDelta: 0, version: 0 },
      loadMorePast,
      loadMoreFuture,
    });

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
    vi.mocked(useCalendarData).mockReturnValue({
      state: 'ready',
      weeks: [
        buildWeek('2026-03-23'),
        buildWeek('2026-03-30'),
        buildWeek('2026-04-06'),
        buildWeek('2026-04-13'),
        buildWeek('2026-04-20'),
      ],
      topPreviewWeek: buildWeek('2026-03-16'),
      bottomPreviewWeek: buildWeek('2026-04-27'),
      isLoadingPast: true,
      isLoadingFuture: false,
      loadingEdge: null,
      scrollAdjustment: { topDelta: 0, version: 0 },
      loadMorePast,
      loadMoreFuture,
    });

    render(<CalendarGrid apiBaseUrl="" />);

    const scroller = screen.getByRole('region', { name: /performance calendar/i });
    Object.defineProperty(scroller, 'scrollTop', { configurable: true, value: 900 });
    Object.defineProperty(scroller, 'clientHeight', { configurable: true, value: 200 });
    Object.defineProperty(scroller, 'scrollHeight', { configurable: true, value: 1000 });

    fireEvent.scroll(scroller);

    expect(loadMoreFuture).not.toHaveBeenCalled();
  });

  it('renders a single loading row above the window when fetching earlier weeks', () => {
    vi.mocked(useCalendarData).mockReturnValue({
      state: 'ready',
      weeks: [
        buildWeek('2026-03-23'),
        buildWeek('2026-03-30'),
        buildWeek('2026-04-06'),
        buildWeek('2026-04-13'),
        buildWeek('2026-04-20'),
      ],
      topPreviewWeek: buildWeek('2026-03-16'),
      bottomPreviewWeek: buildWeek('2026-04-27'),
      isLoadingPast: true,
      isLoadingFuture: false,
      loadingEdge: 'top',
      scrollAdjustment: { topDelta: 0, version: 0 },
      loadMorePast: vi.fn(),
      loadMoreFuture: vi.fn(),
    });

    render(<CalendarGrid apiBaseUrl="" />);

    expect(screen.getByText(/loading week/i)).toBeInTheDocument();
  });
});
