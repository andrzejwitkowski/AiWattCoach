import { render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';

import '../../../i18n';
import { makeActivity, makeCalendarDay, makeEvent } from '../testData';
import { CalendarDayCell } from './CalendarDayCell';

describe('CalendarDayCell content', () => {
  it('renders a rest day state when no data is present', () => {
    render(<CalendarDayCell day={makeCalendarDay({ date: new Date(2026, 2, 23), dateKey: '2026-03-23' })} isToday={false} />);

    expect(screen.getByText(/rest day/i)).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: /rest day/i })).not.toBeInTheDocument();
  });

  it('renders workout content when activity data exists', () => {
    const day = makeCalendarDay({
      date: new Date(2026, 2, 24),
      dateKey: '2026-03-24',
      activities: [
        makeActivity({
          id: 'a1',
          name: 'Morning Ride',
          distanceMeters: 40000,
          movingTimeSeconds: 5400,
          elapsedTimeSeconds: 5600,
          hasHeartRate: true,
          metrics: { trainingStressScore: 88, calories: 950 },
        }),
      ],
    });

    render(<CalendarDayCell day={day} isToday={true} />);

    expect(screen.getByText('Morning Ride')).toBeInTheDocument();
    expect(screen.getByText(/90 min/i)).toBeInTheDocument();
    expect(screen.getByText(/88 TSS/i)).toBeInTheDocument();
  });

  it('shows a more-items indicator when multiple items exist on one day', () => {
    const day = makeCalendarDay({
      date: new Date(2026, 2, 24),
      dateKey: '2026-03-24',
      events: [makeEvent({ id: 2, name: 'Planned ride', indoor: false })],
      activities: [
        makeActivity({
          id: 'a1',
          name: 'Morning Ride',
          distanceMeters: 40000,
          movingTimeSeconds: 5400,
          elapsedTimeSeconds: 5600,
          hasHeartRate: true,
          metrics: { trainingStressScore: 88, calories: 950 },
        }),
      ],
    });

    render(<CalendarDayCell day={day} isToday={false} />);

    expect(screen.getByText('+1 more item')).toBeInTheDocument();
  });

  it('does not label unnamed training activity as a rest day', () => {
    const day = makeCalendarDay({
      activities: [
        makeActivity({
          id: 'a2',
          name: null,
          distanceMeters: 32000,
          movingTimeSeconds: 3600,
          elapsedTimeSeconds: 3700,
          hasHeartRate: true,
          metrics: { trainingStressScore: 55, calories: 700 },
        }),
      ],
    });

    const { container } = render(<CalendarDayCell day={day} isToday={false} />);

    expect(screen.getByText('Ride')).toBeInTheDocument();
    expect(container).not.toHaveTextContent(/^Rest Day$/i);
  });

  it('uses the primary activity type as subtitle fallback when activity metrics are missing', () => {
    const day = makeCalendarDay({
      date: new Date(2026, 2, 27),
      dateKey: '2026-03-27',
      events: [makeEvent({ id: 4, name: 'Planned swim set', category: 'SWIM', indoor: false })],
      activities: [
        makeActivity({
          id: 'a4',
          name: 'Evening Ride',
          distanceMeters: 0,
          movingTimeSeconds: null,
          elapsedTimeSeconds: null,
          hasHeartRate: false,
        }),
      ],
    });

    const { container } = render(<CalendarDayCell day={day} isToday={false} />);

    expect(container).toHaveTextContent('Evening Ride');
    expect(container).toHaveTextContent('Ride');
    expect(container).not.toHaveTextContent('Swim');
  });

  it('does not render a clickable button without a real select handler', () => {
    const day = makeCalendarDay({
      date: new Date(2026, 2, 29),
      dateKey: '2026-03-29',
      events: [makeEvent({ id: 7, name: 'Planned ride', indoor: false })],
    });

    render(<CalendarDayCell day={day} isToday={false} />);

    expect(screen.queryByRole('button', { name: /planned ride/i })).not.toBeInTheDocument();
    expect(screen.getByText('Planned ride')).toBeInTheDocument();
  });

  it('renders a clickable button when a select handler is provided', () => {
    const day = makeCalendarDay({
      events: [makeEvent({ id: 8, name: 'Selectable ride' })],
    });
    const onSelect = vi.fn();

    render(<CalendarDayCell day={day} isToday={false} onSelect={onSelect} />);

    expect(screen.getByRole('button', { name: /selectable ride/i })).toBeInTheDocument();
  });
});
