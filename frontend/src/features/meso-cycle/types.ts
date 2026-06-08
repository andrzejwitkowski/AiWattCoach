import { z } from 'zod';

const mesoCycleWindowSchema = z.object({
  mesoStart: z.string(),
  mesoEnd: z.string(),
  aiCoachLastDate: z.string().nullable(),
});

export const mesoCycleOperationSchema = z.object({
  operationKey: z.string(),
  status: z.enum(['pending', 'completed', 'failed']),
  mesoStart: z.string().nullable(),
  mesoEnd: z.string().nullable(),
  failureMessage: z.string().nullable(),
  updatedAtEpochSeconds: z.number().int(),
});

export const mesoCycleStatusSchema = z.object({
  window: mesoCycleWindowSchema.nullable(),
  hasPendingGeneration: z.boolean(),
  latestOperation: mesoCycleOperationSchema.nullable(),
});

export const mesoCycleCalendarDaySchema = z.object({
  date: z.string(),
  restDay: z.boolean(),
  restDayReason: z.string().nullable(),
  name: z.string().nullable(),
  rawWorkoutDoc: z.string().nullable(),
  overlapStatus: z.enum(['active', 'outdated']),
});

export type MesoCycleStatus = z.infer<typeof mesoCycleStatusSchema>;
export type MesoCycleCalendarDay = z.infer<typeof mesoCycleCalendarDaySchema>;
