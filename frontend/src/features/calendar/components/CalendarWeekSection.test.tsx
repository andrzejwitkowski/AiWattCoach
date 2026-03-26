import { render } from '@testing-library/react';
import { describe, expect, it } from 'vitest';

import '../../../i18n';
import type { CalendarWeek } from '../types';
import { CalendarWeekSection } from './CalendarWeekSection';

describe('CalendarWeekSection', () => {
  it('uses a fixed row height so scroll math matches the rendered week', () => {
    const mondayDate = new Date('2026-03-23T00:00:00');
    const week: CalendarWeek = {
      weekNumber: 12,
      weekKey: '2026-03-23',
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

    const { container } = render(<CalendarWeekSection week={week} />);

    expect(container.querySelector('section')).toHaveClass('h-[320px]');
  });
});
