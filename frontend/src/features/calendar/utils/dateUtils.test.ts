import { describe, expect, it } from 'vitest';

import {
  addWeeks,
  extractDateKey,
  formatDateRange,
  generateWeekDates,
  getMondayOfWeek,
  getWeekNumber,
  parseDateKey,
  toDateKey,
} from './dateUtils';

describe('dateUtils', () => {
  it('returns monday for a midweek date and sunday', () => {
    expect(toDateKey(getMondayOfWeek(new Date(2026, 2, 25)))).toBe('2026-03-23');
    expect(toDateKey(getMondayOfWeek(new Date(2026, 2, 29)))).toBe('2026-03-23');
  });

  it('generates 7 sequential week dates', () => {
    const week = generateWeekDates(new Date(2026, 2, 23));

    expect(week).toHaveLength(7);
    expect(toDateKey(week[0])).toBe('2026-03-23');
    expect(toDateKey(week[6])).toBe('2026-03-29');
  });

  it('formats API date range for multiple weeks', () => {
    expect(formatDateRange(new Date(2026, 2, 23), 5)).toEqual({
      oldest: '2026-03-23',
      newest: '2026-04-26',
    });
  });

  it('parses and extracts date keys', () => {
    expect(toDateKey(parseDateKey('2026-03-25'))).toBe('2026-03-25');
    expect(extractDateKey('2026-03-25T08:00:00')).toBe('2026-03-25');
  });

  it('adds weeks and returns ISO week number', () => {
    expect(toDateKey(addWeeks(new Date(2026, 2, 23), 2))).toBe('2026-04-06');
    expect(getWeekNumber(new Date(2026, 2, 23))).toBeGreaterThan(0);
  });
});
