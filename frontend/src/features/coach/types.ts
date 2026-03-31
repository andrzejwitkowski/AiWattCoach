import type { IntervalEvent } from '../intervals/types';
import { z } from 'zod';

export const conversationMessageRoleSchema = z.enum(['user', 'coach']);

export const conversationMessageSchema = z.object({
  id: z.string(),
  role: conversationMessageRoleSchema,
  content: z.string(),
  createdAtEpochSeconds: z.number().int(),
});

export const workoutSummarySchema = z.object({
  id: z.string(),
  eventId: z.string(),
  rpe: z.number().int().min(1).max(10).nullable(),
  messages: z.array(conversationMessageSchema),
  createdAtEpochSeconds: z.number().int(),
  updatedAtEpochSeconds: z.number().int(),
});

export const sendMessageRequestSchema = z.object({
  content: z.string().trim().min(1),
});

export const updateRpeRequestSchema = z.object({
  rpe: z.number().int().min(1).max(10),
});

export const sendMessageResponseSchema = z.object({
  summary: workoutSummarySchema,
  userMessage: conversationMessageSchema,
  coachMessage: conversationMessageSchema,
});

export const clientWsMessageSchema = z.object({
  type: z.literal('send_message'),
  content: z.string().trim().min(1),
});

export const coachTypingWsMessageSchema = z.object({
  type: z.literal('coach_typing'),
});

export const coachMessageWsMessageSchema = z.object({
  type: z.literal('coach_message'),
  message: conversationMessageSchema,
  summary: workoutSummarySchema,
});

export const errorWsMessageSchema = z.object({
  type: z.literal('error'),
  error: z.string(),
});

export const serverWsMessageSchema = z.discriminatedUnion('type', [
  coachTypingWsMessageSchema,
  coachMessageWsMessageSchema,
  errorWsMessageSchema,
]);

export type ConversationMessage = z.infer<typeof conversationMessageSchema>;
export type WorkoutSummary = z.infer<typeof workoutSummarySchema>;
export type SendMessageResponse = z.infer<typeof sendMessageResponseSchema>;
export type ClientWsMessage = z.infer<typeof clientWsMessageSchema>;
export type ServerWsMessage = z.infer<typeof serverWsMessageSchema>;

export type CoachWorkoutListItem = {
  event: IntervalEvent;
  summary: WorkoutSummary | null;
  hasSummary: boolean;
  hasConversation: boolean;
};
