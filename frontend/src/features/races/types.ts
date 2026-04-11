import { z } from 'zod';

const dateSchema = z.string().regex(/^\d{4}-\d{2}-\d{2}$/);

export const raceDisciplineSchema = z.enum(['road', 'mtb', 'gravel', 'cyclocross', 'timetrial']);
export const racePrioritySchema = z.enum(['A', 'B', 'C']);
export const raceSyncStatusSchema = z.enum(['pending', 'synced', 'failed', 'pending_delete']);
export const raceResultSchema = z.enum(['finished', 'dnf', 'dsq']);

export const listRacesQuerySchema = z.object({
  oldest: dateSchema,
  newest: dateSchema,
});

export const upsertRaceRequestSchema = z.object({
  date: dateSchema,
  name: z.string().trim().min(1),
  distanceMeters: z.number().int().positive(),
  discipline: raceDisciplineSchema,
  priority: racePrioritySchema,
});

export const raceDtoSchema = z.object({
  raceId: z.string(),
  date: dateSchema,
  name: z.string(),
  distanceMeters: z.number().int(),
  discipline: raceDisciplineSchema,
  priority: racePrioritySchema,
  syncStatus: raceSyncStatusSchema,
  linkedIntervalsEventId: z.number().int().nullable(),
  lastError: z.string().nullable(),
  result: raceResultSchema.optional(),
});

export const racesResponseSchema = z.array(raceDtoSchema);

export type Race = z.infer<typeof raceDtoSchema>;
export type RaceDiscipline = z.infer<typeof raceDisciplineSchema>;
export type RacePriority = z.infer<typeof racePrioritySchema>;
export type RaceSyncStatus = z.infer<typeof raceSyncStatusSchema>;
export type RaceResult = z.infer<typeof raceResultSchema>;
export type ListRacesQuery = z.infer<typeof listRacesQuerySchema>;
export type UpsertRaceRequest = z.infer<typeof upsertRaceRequestSchema>;
