import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';

import '../../../i18n';
import type { CalendarWeek } from '../types';
import { CalendarWeekSection } from './CalendarWeekSection';

function createWeek(status: CalendarWeek['status']): CalendarWeek {
  return {
    weekNumber: 12,
    weekKey: '2026-03-23',
    mondayDate: new Date(2026, 2, 23),
    days: Array.from({ length: 7 }, (_, index) => ({
      date: new Date(2026, 2, 23 + index),
      dateKey: `2026-03-${String(23 + index).padStart(2, '0')}`,
      events: [],
      activities: [],
    })),
    summary: {
      totalTss: 0,
      targetTss: null,
      totalCalories: 0,
      totalDurationSeconds: 0,
      targetDurationSeconds: null,
      totalDistanceMeters: 0,
    },
    status,
  };
}

describe('CalendarWeekSection', () => {
  it('renders a quiet placeholder for idle weeks', () => {
    render(<CalendarWeekSection week={createWeek('idle')} />);

    expect(screen.queryByText(/fetching data/i)).not.toBeInTheDocument();
  });

  it('renders spinner label for loading weeks', () => {
    render(<CalendarWeekSection week={createWeek('loading')} />);

    expect(screen.getByText(/fetching data/i)).toBeInTheDocument();
  });
});
