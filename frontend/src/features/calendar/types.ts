import { z } from 'zod';

import type { IntervalActivity, IntervalEvent } from '../intervals/types';
import { raceDisciplineSchema, racePrioritySchema, raceSyncStatusSchema } from '../races/types';

export const calendarCoachMessageRoleSchema = z.enum(['user', 'coach', 'system', 'tool']);

export const calendarCoachToolCallSchema = z.object({
  id: z.string(),
  name: z.string(),
  argumentsJson: z.string(),
});

export const calendarCoachConversationSchema = z.object({
  conversationId: z.string(),
  surface: z.literal('calendar'),
  status: z.enum(['active', 'archived']),
  focus: z.enum(['overview']),
  createdAtEpochSeconds: z.number().int(),
  updatedAtEpochSeconds: z.number().int(),
});

export const calendarCoachMessageSchema = z.object({
  id: z.string(),
  role: calendarCoachMessageRoleSchema,
  content: z.string(),
  toolCall: calendarCoachToolCallSchema.nullish(),
  createdAtEpochSeconds: z.number().int(),
});

const calendarCoachToolOnlyMessageSchema = calendarCoachMessageSchema.extend({
  role: z.literal('tool'),
  toolCall: calendarCoachToolCallSchema,
});

export const calendarCoachConversationResponseSchema = z.object({
  conversation: calendarCoachConversationSchema,
  messages: z.array(calendarCoachMessageSchema),
});

export const calendarCoachSendMessageRequestSchema = z.object({
  content: z.string().trim().min(1).max(2000),
});

export const calendarCoachSendMessageResponseSchema = z.object({
  conversation: calendarCoachConversationSchema,
  messages: z.array(calendarCoachMessageSchema),
  userMessage: calendarCoachMessageSchema,
  coachMessage: calendarCoachMessageSchema,
});

export const calendarCoachClientWsMessageSchema = z.object({
  type: z.literal('send_message'),
  content: z.string().trim().min(1).max(2000),
});

export const calendarCoachTypingWsMessageSchema = z.object({
  type: z.literal('coach_typing'),
});

export const calendarCoachMessageWsMessageSchema = z.object({
  type: z.literal('coach_message'),
  message: calendarCoachMessageSchema,
  conversation: calendarCoachConversationSchema,
  messages: z.array(calendarCoachMessageSchema),
});

export const calendarCoachToolMessageWsMessageSchema = z.object({
  type: z.literal('tool_message'),
  message: calendarCoachToolOnlyMessageSchema,
});

export const calendarCoachSystemMessageWsMessageSchema = z.object({
  type: z.literal('system_message'),
  content: z.string().trim().min(1),
});

export const calendarCoachErrorWsMessageSchema = z.object({
  type: z.literal('error'),
  error: z.string(),
});

export const calendarCoachServerWsMessageSchema = z.discriminatedUnion('type', [
  calendarCoachTypingWsMessageSchema,
  calendarCoachMessageWsMessageSchema,
  calendarCoachToolMessageWsMessageSchema,
  calendarCoachSystemMessageWsMessageSchema,
  calendarCoachErrorWsMessageSchema,
]);

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
export type CalendarCoachConversation = z.infer<typeof calendarCoachConversationSchema>;
export type CalendarCoachMessage = z.infer<typeof calendarCoachMessageSchema>;
export type CalendarCoachConversationResponse = z.infer<typeof calendarCoachConversationResponseSchema>;
export type CalendarCoachSendMessageResponse = z.infer<typeof calendarCoachSendMessageResponseSchema>;
export type CalendarCoachClientWsMessage = z.infer<typeof calendarCoachClientWsMessageSchema>;
export type CalendarCoachServerWsMessage = z.infer<typeof calendarCoachServerWsMessageSchema>;

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
