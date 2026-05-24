import { z } from 'zod';

export const taskStatusSchema = z.enum([
  'queued',
  'running',
  'retry_scheduled',
  'failed',
  'completed',
  'timed_out',
  'cancelled',
]);

export const taskSortFieldSchema = z.enum([
  'id',
  'userId',
  'taskType',
  'status',
  'dedupeKey',
  'errorMessage',
  'attemptCount',
  'nextAttemptAt',
  'claimedBy',
  'leaseExpiresAt',
  'lastHeartbeatAt',
  'executionTimeout',
  'timedOutAt',
  'leaderOnly',
  'createdAt',
  'updatedAt',
  'startedAt',
  'finishedAt',
]);

export const sortDirectionSchema = z.enum(['asc', 'desc']);

const retryStrategySchema = z.discriminatedUnion('kind', [
  z.object({ kind: z.literal('never') }),
  z.object({
    kind: z.literal('fixed'),
    maxAttempts: z.number().int(),
    delaySeconds: z.number().int(),
  }),
  z.object({
    kind: z.literal('exponential'),
    maxAttempts: z.number().int(),
    initialDelaySeconds: z.number().int(),
    maxDelaySeconds: z.number().int(),
  }),
]);

export const scheduledTaskSchema = z.object({
  id: z.string(),
  userId: z.string(),
  taskType: z.string(),
  status: taskStatusSchema,
  payload: z.unknown(),
  checkpoint: z.unknown().nullable(),
  retryStrategy: retryStrategySchema,
  dedupeKey: z.string(),
  errorMessage: z.string().nullable(),
  attemptCount: z.number().int(),
  nextAttemptAtEpochSeconds: z.number().int(),
  claimedBy: z.string().nullable(),
  leaseExpiresAtEpochSeconds: z.number().int().nullable(),
  lastHeartbeatAtEpochSeconds: z.number().int().nullable(),
  executionTimeoutSeconds: z.number().int(),
  timedOutAtEpochSeconds: z.number().int().nullable(),
  leaderOnly: z.boolean(),
  createdAtEpochSeconds: z.number().int(),
  updatedAtEpochSeconds: z.number().int(),
  startedAtEpochSeconds: z.number().int().nullable(),
  finishedAtEpochSeconds: z.number().int().nullable(),
});

export const taskListPageSchema = z.object({
  items: z.array(scheduledTaskSchema),
  nextOffset: z.number().int().nullable(),
  previousOffset: z.number().int().nullable(),
  limit: z.number().int(),
});

export type ScheduledTask = z.infer<typeof scheduledTaskSchema>;
export type TaskListPage = z.infer<typeof taskListPageSchema>;
export type TaskSortField = z.infer<typeof taskSortFieldSchema>;
export type SortDirection = z.infer<typeof sortDirectionSchema>;
export type TaskStatus = z.infer<typeof taskStatusSchema>;

export type TaskListParams = {
  limit: number;
  offset: number;
  sortField: TaskSortField;
  sortDirection: SortDirection;
};
