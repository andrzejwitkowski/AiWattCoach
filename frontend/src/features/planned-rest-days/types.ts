import { z } from 'zod';

const dateSchema = z.string().regex(/^\d{4}-\d{2}-\d{2}$/);

export const listPlannedRestDaysQuerySchema = z.object({
  oldest: dateSchema,
  newest: dateSchema,
});

export const upsertPlannedRestDayRequestSchema = z
  .object({
    startDate: dateSchema,
    endDate: dateSchema,
    title: z.string().trim().max(120).nullable().optional(),
    note: z.string().trim().max(2000).nullable().optional(),
  })
  .refine((value) => value.endDate >= value.startDate, {
    message: 'endBeforeStart',
    path: ['endDate'],
  });

export const plannedRestDaySchema = z.object({
  plannedRestDayId: z.string(),
  startDate: dateSchema,
  endDate: dateSchema,
  title: z.string().nullable(),
  note: z.string().nullable(),
  createdAtEpochSeconds: z.number().int(),
  updatedAtEpochSeconds: z.number().int(),
});

export const plannedRestDaysResponseSchema = z.array(plannedRestDaySchema);

export type PlannedRestDay = z.infer<typeof plannedRestDaySchema>;
export type ListPlannedRestDaysQuery = z.infer<typeof listPlannedRestDaysQuerySchema>;
export type UpsertPlannedRestDayRequest = z.infer<typeof upsertPlannedRestDayRequestSchema>;
