import { z } from 'zod';

const jsonValueSchema: z.ZodType<unknown> = z.lazy(() =>
  z.union([z.string(), z.number(), z.boolean(), z.null(), z.array(jsonValueSchema), z.record(z.string(), jsonValueSchema)])
);

export const intervalDefinitionSchema = z.object({
  definition: z.string(),
  repeatCount: z.number().int(),
  durationSeconds: z.number().int().nullable(),
  targetPercentFtp: z.number().nullable(),
  zoneId: z.number().int().nullable(),
});

export const workoutSegmentSchema = z.object({
  order: z.number().int(),
  label: z.string(),
  durationSeconds: z.number().int(),
  startOffsetSeconds: z.number().int(),
  endOffsetSeconds: z.number().int(),
  targetPercentFtp: z.number().nullable(),
  zoneId: z.number().int().nullable(),
});

export const workoutSummarySchema = z.object({
  totalSegments: z.number().int(),
  totalDurationSeconds: z.number().int(),
  estimatedNormalizedPowerWatts: z.number().int().nullable(),
  estimatedAveragePowerWatts: z.number().int().nullable(),
  estimatedIntensityFactor: z.number().nullable(),
  estimatedTrainingStressScore: z.number().nullable(),
});

export const eventDefinitionSchema = z.object({
  rawWorkoutDoc: z.string().nullable(),
  intervals: z.array(intervalDefinitionSchema),
  segments: z.array(workoutSegmentSchema),
  summary: workoutSummarySchema,
});

export const matchedWorkoutIntervalSchema = z.object({
  plannedSegmentOrder: z.number().int(),
  plannedLabel: z.string(),
  plannedDurationSeconds: z.number().int(),
  targetPercentFtp: z.number().nullable(),
  zoneId: z.number().int().nullable(),
  actualIntervalId: z.number().int().nullable(),
  actualStartTimeSeconds: z.number().int().nullable(),
  actualEndTimeSeconds: z.number().int().nullable(),
  averagePowerWatts: z.number().int().nullable(),
  normalizedPowerWatts: z.number().int().nullable(),
  averageHeartRateBpm: z.number().int().nullable(),
  averageCadenceRpm: z.number().nullable(),
  averageSpeedMps: z.number().nullable(),
  complianceScore: z.number(),
});

export const actualWorkoutSchema = z.object({
  activityId: z.string(),
  activityName: z.string().nullable(),
  startDateLocal: z.string(),
  powerValues: z.array(z.number().int()),
  cadenceValues: z.array(z.number().int()),
  heartRateValues: z.array(z.number().int()),
  speedValues: z.array(z.number()),
  averagePowerWatts: z.number().int().nullable(),
  normalizedPowerWatts: z.number().int().nullable(),
  trainingStressScore: z.number().int().nullable(),
  intensityFactor: z.number().nullable(),
  complianceScore: z.number(),
  matchedIntervals: z.array(matchedWorkoutIntervalSchema),
});

export const eventFileUploadSchema = z.object({
  filename: z.string(),
  fileContents: z.string().optional(),
  fileContentsBase64: z.string().optional(),
}).refine(
  (value) => {
    const hasFileContents = Boolean(value.fileContents?.trim());
    const hasFileContentsBase64 = Boolean(value.fileContentsBase64?.trim());
    return hasFileContents !== hasFileContentsBase64;
  },
  {
    message: 'Exactly one of fileContents or fileContentsBase64 must be provided.'
  }
);

export const intervalEventSchema = z.object({
  id: z.number().int(),
  calendarEntryId: z.string().optional(),
  startDateLocal: z.string(),
  name: z.string().nullable(),
  category: z.string(),
  description: z.string().nullable(),
  indoor: z.boolean(),
  color: z.string().nullable(),
  eventDefinition: eventDefinitionSchema,
  actualWorkout: actualWorkoutSchema.nullable().default(null),
  plannedSource: z.enum(['intervals', 'predicted']).optional(),
  syncStatus: z.enum(['unsynced', 'pending', 'synced', 'modified', 'failed']).nullable().optional(),
  linkedIntervalsEventId: z.number().int().nullable().optional(),
  projectedWorkout: z.object({
    projectedWorkoutId: z.string(),
    operationKey: z.string(),
    date: z.string(),
    sourceWorkoutId: z.string(),
  }).nullable().optional(),
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
  fileUpload: eventFileUploadSchema.optional(),
});

export const updateIntervalEventRequestSchema = z.object({
  category: intervalCategorySchema.optional(),
  startDateLocal: dateSchema.optional(),
  name: z.string().optional(),
  description: z.string().optional(),
  indoor: z.boolean().optional(),
  color: z.string().optional(),
  workoutDoc: z.string().optional(),
  fileUpload: eventFileUploadSchema.optional(),
});

export const activityMetricsSchema = z.object({
  trainingStressScore: z.number().int().nullable(),
  normalizedPowerWatts: z.number().int().nullable(),
  intensityFactor: z.number().nullable(),
  efficiencyFactor: z.number().nullable(),
  variabilityIndex: z.number().nullable(),
  averagePowerWatts: z.number().int().nullable(),
  ftpWatts: z.number().int().nullable(),
  totalWorkJoules: z.number().int().nullable(),
  calories: z.number().int().nullable(),
  trimp: z.number().nullable(),
  powerLoad: z.number().int().nullable(),
  heartRateLoad: z.number().int().nullable(),
  paceLoad: z.number().int().nullable(),
  strainScore: z.number().nullable(),
});

export const activityIntervalSchema = z.object({
  id: z.number().int().nullable(),
  label: z.string().nullable(),
  intervalType: z.string().nullable(),
  groupId: z.string().nullable(),
  startIndex: z.number().int().nullable(),
  endIndex: z.number().int().nullable(),
  startTimeSeconds: z.number().int().nullable(),
  endTimeSeconds: z.number().int().nullable(),
  movingTimeSeconds: z.number().int().nullable(),
  elapsedTimeSeconds: z.number().int().nullable(),
  distanceMeters: z.number().nullable(),
  averagePowerWatts: z.number().int().nullable(),
  normalizedPowerWatts: z.number().int().nullable(),
  trainingStressScore: z.number().nullable(),
  averageHeartRateBpm: z.number().int().nullable(),
  averageCadenceRpm: z.number().nullable(),
  averageSpeedMps: z.number().nullable(),
  averageStrideMeters: z.number().nullable(),
  zone: z.number().int().nullable(),
});

export const activityIntervalGroupSchema = z.object({
  id: z.string(),
  count: z.number().int().nullable(),
  startIndex: z.number().int().nullable(),
  movingTimeSeconds: z.number().int().nullable(),
  elapsedTimeSeconds: z.number().int().nullable(),
  distanceMeters: z.number().nullable(),
  averagePowerWatts: z.number().int().nullable(),
  normalizedPowerWatts: z.number().int().nullable(),
  trainingStressScore: z.number().nullable(),
  averageHeartRateBpm: z.number().int().nullable(),
  averageCadenceRpm: z.number().nullable(),
  averageSpeedMps: z.number().nullable(),
  averageStrideMeters: z.number().nullable(),
});

export const activityStreamSchema = z.object({
  streamType: z.string(),
  name: z.string().nullable(),
  data: jsonValueSchema.nullable(),
  data2: jsonValueSchema.nullable(),
  valueTypeIsArray: z.boolean(),
  custom: z.boolean(),
  allNull: z.boolean(),
});

export const activityZoneTimeSchema = z.object({
  zoneId: z.string(),
  seconds: z.number().int(),
});

export const activityDetailsSchema = z.object({
  intervals: z.array(activityIntervalSchema),
  intervalGroups: z.array(activityIntervalGroupSchema),
  streams: z.array(activityStreamSchema),
  intervalSummary: z.array(z.string()),
  skylineChart: z.array(z.string()),
  powerZoneTimes: z.array(activityZoneTimeSchema),
  heartRateZoneTimes: z.array(z.number().int()),
  paceZoneTimes: z.array(z.number().int()),
  gapZoneTimes: z.array(z.number().int()),
});

export const intervalActivitySchema = z.object({
  id: z.string(),
  startDateLocal: z.string(),
  startDate: z.string().nullable(),
  name: z.string().nullable(),
  description: z.string().nullable(),
  activityType: z.string().nullable(),
  source: z.string().nullable(),
  externalId: z.string().nullable(),
  deviceName: z.string().nullable(),
  distanceMeters: z.number().nullable(),
  movingTimeSeconds: z.number().int().nullable(),
  elapsedTimeSeconds: z.number().int().nullable(),
  totalElevationGainMeters: z.number().nullable(),
  averageSpeedMps: z.number().nullable(),
  averageHeartRateBpm: z.number().int().nullable(),
  averageCadenceRpm: z.number().nullable(),
  trainer: z.boolean(),
  commute: z.boolean(),
  race: z.boolean(),
  hasHeartRate: z.boolean(),
  streamTypes: z.array(z.string()),
  tags: z.array(z.string()),
  metrics: activityMetricsSchema,
  details: activityDetailsSchema,
  detailsUnavailableReason: z.string().nullable().optional(),
});

export const intervalActivitiesResponseSchema = z.array(intervalActivitySchema);

export const uploadActivityRequestSchema = z.object({
  filename: z.string(),
  fileContentsBase64: z.string(),
  name: z.string().optional(),
  description: z.string().optional(),
  deviceName: z.string().optional(),
  externalId: z.string().optional(),
  pairedEventId: z.number().int().optional(),
});

export const uploadActivityResponseSchema = z.object({
  created: z.boolean(),
  activityIds: z.array(z.string()),
  activities: z.array(intervalActivitySchema),
});

export const updateActivityRequestSchema = z.object({
  name: z.string().optional(),
  description: z.string().optional(),
  activityType: z.string().optional(),
  trainer: z.boolean().optional(),
  commute: z.boolean().optional(),
  race: z.boolean().optional(),
});

export type IntervalEvent = z.infer<typeof intervalEventSchema>;
export type IntervalActivity = z.infer<typeof intervalActivitySchema>;
export type ListEventsQuery = z.infer<typeof listEventsQuerySchema>;
export type CreateIntervalEventRequest = z.infer<typeof createIntervalEventRequestSchema>;
export type UpdateIntervalEventRequest = z.infer<typeof updateIntervalEventRequestSchema>;
export type UploadActivityRequest = z.infer<typeof uploadActivityRequestSchema>;
export type UpdateActivityRequest = z.infer<typeof updateActivityRequestSchema>;
