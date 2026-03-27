import type { IntervalActivity, IntervalEvent } from '../intervals/types';

export type CalendarWeekStatus = 'idle' | 'loading' | 'loaded' | 'error';

export type CalendarDay = {
  date: Date;
  dateKey: string;
  events: IntervalEvent[];
  activities: IntervalActivity[];
};

export type CalendarWeekSummary = {
  totalTss: number;
  targetTss: number | null;
  totalCalories: number;
  totalDurationSeconds: number;
  targetDurationSeconds: number | null;
  totalDistanceMeters: number;
};

export type CalendarWeek = {
  weekNumber: number;
  weekKey: string;
  mondayDate: Date;
  days: CalendarDay[];
  summary: CalendarWeekSummary;
  status: CalendarWeekStatus;
};

export type CalendarDataState = 'loading' | 'ready' | 'credentials-required' | 'error';

export type CalendarScrollAdjustment = {
  topDelta: number;
  version: number;
};
