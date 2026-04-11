import { z } from 'zod';

import type { IntervalActivity, IntervalEvent } from '../intervals/types';
import { raceDisciplineSchema, racePrioritySchema, raceSyncStatusSchema } from '../races/types';

export const calendarRaceLabelPayloadSchema = z.object({
  raceId: z.string(),
  date: z.string().regex(/^\d{4}-\d{2}-\d{2}$/),
  name: z.string(),
  distanceMeters: z.number().int(),
  discipline: raceDisciplineSchema,
  priority: racePrioritySchema,
  syncStatus: raceSyncStatusSchema,
  linkedIntervalsEventId: z.number().int().nullable(),
});

export const calendarActivityLabelPayloadSchema = z.object({
  labelId: z.string(),
  activityKind: z.string(),
  note: z.string().nullable(),
});

export const calendarHealthLabelPayloadSchema = z.object({
  labelId: z.string(),
  status: z.string(),
  note: z.string().nullable(),
});

export const calendarCustomLabelPayloadSchema = z.object({
  labelId: z.string(),
  value: z.string(),
});

export const calendarLabelSchema = z.discriminatedUnion('kind', [
  z.object({
    kind: z.literal('race'),
    title: z.string(),
    subtitle: z.string().nullable(),
    payload: calendarRaceLabelPayloadSchema,
  }),
  z.object({
    kind: z.literal('activity'),
    title: z.string(),
    subtitle: z.string().nullable(),
    payload: calendarActivityLabelPayloadSchema,
  }),
  z.object({
    kind: z.literal('health'),
    title: z.string(),
    subtitle: z.string().nullable(),
    payload: calendarHealthLabelPayloadSchema,
  }),
  z.object({
    kind: z.literal('custom'),
    title: z.string(),
    subtitle: z.string().nullable(),
    payload: calendarCustomLabelPayloadSchema,
  }),
]);

export const calendarLabelsResponseSchema = z.object({
  labelsByDate: z.record(z.string(), z.record(z.string(), calendarLabelSchema)),
});

export type CalendarLabel = z.infer<typeof calendarLabelSchema>;
export type CalendarRaceLabel = Extract<CalendarLabel, { kind: 'race' }>;

export type CalendarWeekStatus = 'idle' | 'loading' | 'loaded' | 'error';

export type CalendarDay = {
  date: Date;
  dateKey: string;
  events: IntervalEvent[];
  activities: IntervalActivity[];
  labels: CalendarLabel[];
};

export type PlannedWorkoutSyncStatus = 'unsynced' | 'pending' | 'synced' | 'modified' | 'failed';

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
