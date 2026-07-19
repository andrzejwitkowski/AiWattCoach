import type { IntervalActivity, IntervalEvent } from '../intervals/types';
import { z } from 'zod';

export const conversationMessageRoleSchema = z.enum(['user', 'coach', 'system', 'tool']);

export const toolCallSchema = z.object({
  id: z.string(),
  name: z.string(),
  argumentsJson: z.string(),
  argumentsPreview: z.string().nullish(),
});

export const coachQuestionSchema = z.object({
  id: z.string(),
  question: z.string(),
  answers: z.array(z.string().trim().min(1)).min(2).max(6),
  freeTextLabel: z.string().trim().min(1).nullish(),
});

export const conversationMessageSchema = z.object({
  id: z.string(),
  role: conversationMessageRoleSchema,
  content: z.string(),
  toolCall: toolCallSchema.nullish(),
  questions: z.array(coachQuestionSchema).nullish(),
  createdAtEpochSeconds: z.number().int(),
  imageUrl: z.string().nullish(),
});

const toolConversationMessageSchema = conversationMessageSchema.extend({
  role: z.literal('tool'),
  toolCall: toolCallSchema,
});

export const workoutSummarySchema = z.object({
  id: z.string(),
  workoutId: z.string(),
  rpe: z.number().int().min(1).max(10).nullable(),
  hasCoachMessage: z.boolean().optional(),
  messages: z.array(conversationMessageSchema).default([]),
  savedAtEpochSeconds: z.number().int().nullable(),
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

export const saveWorkoutSummaryResponseSchema = z.object({
  summary: workoutSummarySchema,
  workflow: z.object({
    recapStatus: z.enum(['generated', 'processing', 'skipped', 'failed', 'unchanged']),
    planStatus: z.enum(['generated', 'processing', 'skipped', 'failed', 'unchanged']),
    messages: z.array(z.string()),
  }),
});

const saveWorkflowSchema = saveWorkoutSummaryResponseSchema.shape.workflow;

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

export const toolMessageWsMessageSchema = z.object({
  type: z.literal('tool_message'),
  message: toolConversationMessageSchema,
});

export const systemMessageWsMessageSchema = z.object({
  type: z.literal('system_message'),
  content: z.string().trim().min(1),
});

export const errorWsMessageSchema = z.object({
  type: z.literal('error'),
  error: z.string(),
});

export const saveWorkflowCompleteWsMessageSchema = z.object({
  type: z.literal('save_workflow_complete'),
  workflow: saveWorkflowSchema,
});

export const serverWsMessageSchema = z.discriminatedUnion('type', [
  coachTypingWsMessageSchema,
  coachMessageWsMessageSchema,
  toolMessageWsMessageSchema,
  systemMessageWsMessageSchema,
  errorWsMessageSchema,
  saveWorkflowCompleteWsMessageSchema,
]);

export type ConversationMessage = z.infer<typeof conversationMessageSchema>;
export type CoachQuestion = z.infer<typeof coachQuestionSchema>;
export type WorkoutSummary = z.infer<typeof workoutSummarySchema>;
export type SendMessageResponse = z.infer<typeof sendMessageResponseSchema>;
export type SaveWorkoutSummaryResponse = z.infer<typeof saveWorkoutSummaryResponseSchema>;
export type ClientWsMessage = z.infer<typeof clientWsMessageSchema>;
export type ServerWsMessage = z.infer<typeof serverWsMessageSchema>;
export type CoachChatProgressState = 'idle' | 'awaiting-reply' | 'saving-summary';

export type CoachWorkoutListItem = {
  id: string;
  source: 'activity' | 'event';
  startDateLocal: string;
  event: IntervalEvent | null;
  activity: IntervalActivity | null;
  summary: WorkoutSummary | null;
  hasSummary: boolean;
  hasConversation: boolean;
};
