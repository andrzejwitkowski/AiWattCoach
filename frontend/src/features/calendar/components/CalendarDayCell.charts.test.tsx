import { render } from '@testing-library/react';
import { describe, expect, it } from 'vitest';

import '../../../i18n';
import {
  makeActivity,
  makeCalendarDay,
  makeEvent,
  makeEventDefinition,
  makeIntervalDefinition,
  makeWorkoutSegment,
  makeWorkoutSummary,
} from '../testData';
import { CalendarDayCell } from './CalendarDayCell';

describe('CalendarDayCell charts', () => {
  it('keeps the mini chart aligned with the displayed primary activity', () => {
    const day = makeCalendarDay({
      date: new Date(2026, 2, 26),
      dateKey: '2026-03-26',
      events: [
        makeEvent({
          id: 3,
          name: 'Planned intervals',
          indoor: false,
          eventDefinition: makeEventDefinition({
            intervals: [
              makeIntervalDefinition({ definition: '10m' }),
              makeIntervalDefinition({ definition: '8m' }),
              makeIntervalDefinition({ definition: '6m' }),
              makeIntervalDefinition({ definition: '4m' }),
            ],
          }),
        }),
      ],
      activities: [
        makeActivity({
          id: 'a3',
          name: 'Morning Ride',
          distanceMeters: 40000,
          movingTimeSeconds: 5400,
          elapsedTimeSeconds: 5600,
          hasHeartRate: true,
          metrics: { trainingStressScore: 88, calories: 950 },
        }),
      ],
    });

    const { container } = render(<CalendarDayCell day={day} isToday={false} />);

    expect(container).toHaveTextContent('Morning Ride');
    expect(container.querySelectorAll('[data-chart-bar="mini"]').length).toBe(4);
  });

  it('renders planned mini-chart bars with their workout zone colors', () => {
    const day = makeCalendarDay({
      date: new Date(2026, 2, 28),
      dateKey: '2026-03-28',
      events: [
        makeEvent({
          id: 5,
          name: 'Threshold set',
          eventDefinition: makeEventDefinition({
            rawWorkoutDoc: '10m Z2\n5m Z4\n2m Z6',
            intervals: [
              makeIntervalDefinition({ definition: '10m Z2', durationSeconds: 600, targetPercentFtp: 70, zoneId: 2 }),
              makeIntervalDefinition({ definition: '5m Z4', durationSeconds: 300, targetPercentFtp: 100, zoneId: 4 }),
              makeIntervalDefinition({ definition: '2m Z6', durationSeconds: 120, targetPercentFtp: 130, zoneId: 6 }),
            ],
            segments: [
              makeWorkoutSegment({ order: 1, label: 'Warmup', durationSeconds: 600, endOffsetSeconds: 600, targetPercentFtp: 70, zoneId: 2 }),
              makeWorkoutSegment({ order: 2, label: 'Threshold', durationSeconds: 300, startOffsetSeconds: 600, endOffsetSeconds: 900, targetPercentFtp: 100, zoneId: 4 }),
              makeWorkoutSegment({ order: 3, label: 'VO2', durationSeconds: 120, startOffsetSeconds: 900, endOffsetSeconds: 1020, targetPercentFtp: 130, zoneId: 6 }),
            ],
            summary: makeWorkoutSummary({ totalSegments: 3, totalDurationSeconds: 1020, estimatedIntensityFactor: 0.88, estimatedTrainingStressScore: 75 }),
          }),
        }),
      ],
    });

    const { container } = render(<CalendarDayCell day={day} isToday={false} />);
    const chartBars = Array.from(container.querySelectorAll('[data-chart-bar="mini"]')) as HTMLDivElement[];
    const backgroundColors = chartBars.map((bar) => bar.style.backgroundColor).filter(Boolean);

    expect(chartBars.length).toBe(3);
    expect(new Set(backgroundColors).size).toBeGreaterThan(1);
    expect(backgroundColors).toContain('rgb(0, 227, 253)');
    expect(backgroundColors).toContain('rgb(210, 255, 154)');
    expect(backgroundColors).toContain('rgb(255, 115, 81)');
  });

  it('renders mini-chart widths proportional to planned interval duration', () => {
    const day = makeCalendarDay({
      date: new Date(2026, 2, 28),
      dateKey: '2026-03-28',
      events: [
        makeEvent({
          id: 6,
          name: 'Mixed set',
          eventDefinition: makeEventDefinition({
            segments: [
              makeWorkoutSegment({ order: 0, label: 'Long', durationSeconds: 1200, endOffsetSeconds: 1200, targetPercentFtp: 75, zoneId: 3 }),
              makeWorkoutSegment({ order: 1, label: 'Short', durationSeconds: 300, startOffsetSeconds: 1200, endOffsetSeconds: 1500, targetPercentFtp: 110, zoneId: 5 }),
            ],
            summary: makeWorkoutSummary({ totalSegments: 2, totalDurationSeconds: 1500 }),
          }),
        }),
      ],
    });

    const { container } = render(<CalendarDayCell day={day} isToday={false} />);
    const [firstBar, secondBar] = Array.from(container.querySelectorAll('[data-chart-bar="mini"]')) as HTMLDivElement[];

    expect(firstBar.style.flexGrow).toBe('1200');
    expect(secondBar.style.flexGrow).toBe('300');
  });
});
