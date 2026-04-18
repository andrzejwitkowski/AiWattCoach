import { z } from 'zod';

export const dashboardRangeSchema = z.enum(['90d', 'season', 'all-time']);
export const dashboardTsbZoneSchema = z.enum(['freshness_peak', 'optimal_training', 'high_risk']);

export const trainingLoadDashboardPointSchema = z.object({
  date: z.string(),
  dailyTss: z.number().int().nullable(),
  currentCtl: z.number().nullable(),
  currentAtl: z.number().nullable(),
  currentTsb: z.number().nullable(),
});

export const trainingLoadDashboardSummarySchema = z.object({
  currentCtl: z.number().nullable(),
  currentAtl: z.number().nullable(),
  currentTsb: z.number().nullable(),
  ftpWatts: z.number().int().nullable(),
  averageIf28d: z.number().nullable(),
  averageEf28d: z.number().nullable(),
  loadDeltaCtl14d: z.number().nullable(),
  tsbZone: dashboardTsbZoneSchema,
});

export const trainingLoadDashboardResponseSchema = z.object({
  range: dashboardRangeSchema,
  windowStart: z.string(),
  windowEnd: z.string(),
  hasTrainingLoad: z.boolean(),
  summary: trainingLoadDashboardSummarySchema,
  points: z.array(trainingLoadDashboardPointSchema),
});

export type DashboardRange = z.infer<typeof dashboardRangeSchema>;
export type TrainingLoadDashboardResponse = z.infer<typeof trainingLoadDashboardResponseSchema>;
