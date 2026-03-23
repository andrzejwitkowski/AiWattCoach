import { z } from 'zod';

export const intervalDefinitionSchema = z.object({
  definition: z.string(),
});

export const eventDefinitionSchema = z.object({
  rawWorkoutDoc: z.string().nullable(),
  intervals: z.array(intervalDefinitionSchema),
});

export const actualWorkoutSchema = z.object({
  powerValues: z.array(z.number().int()),
  cadenceValues: z.array(z.number().int()),
  heartRateValues: z.array(z.number().int()),
});

export const intervalEventSchema = z.object({
  id: z.number().int(),
  startDateLocal: z.string(),
  name: z.string().nullable(),
  category: z.string(),
  description: z.string().nullable(),
  indoor: z.boolean(),
  color: z.string().nullable(),
  eventDefinition: eventDefinitionSchema,
  actualWorkout: actualWorkoutSchema.nullable(),
});

export const intervalEventsResponseSchema = z.array(intervalEventSchema);

const dateSchema = z.string().regex(/^\d{4}-\d{2}-\d{2}$/);
const intervalCategorySchema = z.enum(['WORKOUT', 'RACE', 'NOTE', 'TARGET', 'SEASON', 'OTHER']);

export const listEventsQuerySchema = z.object({
  oldest: dateSchema,
  newest: dateSchema,
});

export const createIntervalEventRequestSchema = z.object({
  category: intervalCategorySchema,
  startDateLocal: dateSchema,
  name: z.string().optional(),
  description: z.string().optional(),
  indoor: z.boolean().optional(),
  color: z.string().optional(),
  workoutDoc: z.string().optional(),
});

export const updateIntervalEventRequestSchema = z.object({
  category: intervalCategorySchema.optional(),
  startDateLocal: dateSchema.optional(),
  name: z.string().optional(),
  description: z.string().optional(),
  indoor: z.boolean().optional(),
  color: z.string().optional(),
  workoutDoc: z.string().optional(),
});

export type IntervalEvent = z.infer<typeof intervalEventSchema>;
export type ListEventsQuery = z.infer<typeof listEventsQuerySchema>;
export type CreateIntervalEventRequest = z.infer<typeof createIntervalEventRequestSchema>;
export type UpdateIntervalEventRequest = z.infer<typeof updateIntervalEventRequestSchema>;
