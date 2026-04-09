import { z } from 'zod';

export const llmProviderSchema = z.enum(['openai', 'gemini', 'openrouter']);
const availabilityWeekdaySchema = z.enum(['mon', 'tue', 'wed', 'thu', 'fri', 'sat', 'sun']);

const aiAgentsSettingsSchema = z.object({
  openaiApiKey: z.string().nullable(),
  openaiApiKeySet: z.boolean(),
  geminiApiKey: z.string().nullable(),
  geminiApiKeySet: z.boolean(),
  openrouterApiKey: z.string().nullable(),
  openrouterApiKeySet: z.boolean(),
  selectedProvider: llmProviderSchema.nullable().optional(),
  selectedModel: z.string().nullable().optional(),
});

const intervalsSettingsSchema = z.object({
  apiKey: z.string().nullable(),
  apiKeySet: z.boolean(),
  athleteId: z.string().nullable(),
  connected: z.boolean(),
});

export const testIntervalsConnectionResponseSchema = z.object({
  connected: z.boolean(),
  message: z.string(),
  usedSavedApiKey: z.boolean(),
  usedSavedAthleteId: z.boolean(),
  persistedStatusUpdated: z.boolean(),
});

export const testAiAgentsConnectionResponseSchema = z.object({
  connected: z.boolean(),
  message: z.string(),
  usedSavedApiKey: z.boolean(),
  usedSavedProvider: z.boolean(),
  usedSavedModel: z.boolean(),
});

const analysisOptionsSettingsSchema = z.object({
  analyzeWithoutHeartRate: z.boolean(),
});

export const availabilityDaySchema = z.object({
  weekday: availabilityWeekdaySchema,
  available: z.boolean(),
  maxDurationMinutes: z.number().int().nullable(),
}).superRefine((value, ctx) => {
  const allowedMinutes = [30, 60, 90, 120, 150, 180, 210, 240, 270, 300];

  if (value.available && value.maxDurationMinutes === null) {
    ctx.addIssue({
      code: z.ZodIssueCode.custom,
      message: 'Available days require maxDurationMinutes',
      path: ['maxDurationMinutes'],
    });
  }

  if (!value.available && value.maxDurationMinutes !== null) {
    ctx.addIssue({
      code: z.ZodIssueCode.custom,
      message: 'Unavailable days must not define maxDurationMinutes',
      path: ['maxDurationMinutes'],
    });
  }

  if (
    value.available
    && value.maxDurationMinutes !== null
    && !allowedMinutes.includes(value.maxDurationMinutes)
  ) {
    ctx.addIssue({
      code: z.ZodIssueCode.custom,
      message: 'Available days must use a supported duration step',
      path: ['maxDurationMinutes'],
    });
  }
});

function validateExplicitWeek(days: AvailabilityDay[], ctx: z.RefinementCtx) {
  const distinctWeekdays = new Set(days.map((day) => day.weekday));

  if (distinctWeekdays.size !== 7) {
    ctx.addIssue({
      code: z.ZodIssueCode.custom,
      message: 'Availability must include each weekday exactly once',
      path: ['days'],
    });
  }
}

const availabilitySettingsSchema = z.object({
  configured: z.boolean(),
  days: z.array(availabilityDaySchema).length(7),
}).superRefine((value, ctx) => {
  validateExplicitWeek(value.days, ctx);
});

const cyclingSettingsDataSchema = z.object({
  fullName: z.string().nullable(),
  age: z.number().nullable(),
  heightCm: z.number().nullable(),
  weightKg: z.number().nullable(),
  ftpWatts: z.number().nullable(),
  hrMaxBpm: z.number().nullable(),
  vo2Max: z.number().nullable(),
  athletePrompt: z.string().nullable(),
  medications: z.string().nullable(),
  athleteNotes: z.string().nullable(),
  lastZoneUpdateEpochSeconds: z.number().nullable(),
});

export const userSettingsResponseSchema = z.object({
  aiAgents: aiAgentsSettingsSchema,
  intervals: intervalsSettingsSchema,
  options: analysisOptionsSettingsSchema,
  availability: availabilitySettingsSchema,
  cycling: cyclingSettingsDataSchema,
});

export const athleteSummaryResponseSchema = z.object({
  exists: z.boolean(),
  stale: z.boolean(),
  summaryText: z.string().nullable().optional(),
  generatedAtEpochSeconds: z.number().int().nullable().optional(),
  updatedAtEpochSeconds: z.number().int().nullable().optional(),
});

export type UserSettingsResponse = z.infer<typeof userSettingsResponseSchema>;
export type AthleteSummaryResponse = z.infer<typeof athleteSummaryResponseSchema>;

export const updateAiAgentsRequestSchema = z.object({
  openaiApiKey: z.string().nullable().optional(),
  geminiApiKey: z.string().nullable().optional(),
  openrouterApiKey: z.string().nullable().optional(),
  selectedProvider: z.union([llmProviderSchema, z.literal('')]).nullable().optional(),
  selectedModel: z.string().nullable().optional(),
});

export type LlmProvider = z.infer<typeof llmProviderSchema>;

export type UpdateAiAgentsRequest = z.infer<typeof updateAiAgentsRequestSchema>;

export const updateIntervalsRequestSchema = z.object({
  apiKey: z.string().nullable().optional(),
  athleteId: z.string().nullable().optional(),
});

export type UpdateIntervalsRequest = z.infer<typeof updateIntervalsRequestSchema>;

export type TestIntervalsConnectionResponse = z.infer<typeof testIntervalsConnectionResponseSchema>;
export type TestAiAgentsConnectionResponse = z.infer<typeof testAiAgentsConnectionResponseSchema>;

export const updateOptionsRequestSchema = z.object({
  analyzeWithoutHeartRate: z.boolean().optional(),
});

export type UpdateOptionsRequest = z.infer<typeof updateOptionsRequestSchema>;

export const updateAvailabilityRequestSchema = z.object({
  days: z.array(availabilityDaySchema).length(7),
}).superRefine((value, ctx) => {
  validateExplicitWeek(value.days, ctx);
});

export type UpdateAvailabilityRequest = z.infer<typeof updateAvailabilityRequestSchema>;

export const updateCyclingRequestSchema = z.object({
  fullName: z.string().nullable().optional(),
  age: z.number().int().positive().max(120).nullable().optional(),
  heightCm: z.number().int().positive().max(300).nullable().optional(),
  weightKg: z.number().positive().max(500).nullable().optional(),
  ftpWatts: z.number().int().positive().max(2500).nullable().optional(),
  hrMaxBpm: z.number().int().positive().max(300).nullable().optional(),
  vo2Max: z.number().positive().max(100).nullable().optional(),
  athletePrompt: z.string().max(6000).nullable().optional(),
  medications: z.string().max(4000).nullable().optional(),
  athleteNotes: z.string().max(8000).nullable().optional(),
});

export type UpdateCyclingRequest = z.infer<typeof updateCyclingRequestSchema>;

export type CyclingSettingsData = z.infer<typeof cyclingSettingsDataSchema>;
export type AvailabilitySettingsData = z.infer<typeof availabilitySettingsSchema>;
export type AvailabilityDay = z.infer<typeof availabilityDaySchema>;

export function hasExplicitAvailabilityWeek(days: AvailabilityDay[]): boolean {
  return days.length === 7 && new Set(days.map((day) => day.weekday)).size === 7;
}

export function isAvailabilityConfigured(availability: AvailabilitySettingsData | null | undefined): boolean {
  if (!availability) {
    return false;
  }

  return availability.configured && hasExplicitAvailabilityWeek(availability.days) && availability.days.some((day) => day.available);
}
