import { z } from 'zod';

export const llmProviderSchema = z.enum(['openai', 'gemini', 'openrouter']);

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

const cyclingSettingsDataSchema = z.object({
  fullName: z.string().nullable(),
  age: z.number().nullable(),
  heightCm: z.number().nullable(),
  weightKg: z.number().nullable(),
  ftpWatts: z.number().nullable(),
  hrMaxBpm: z.number().nullable(),
  vo2Max: z.number().nullable(),
  lastZoneUpdateEpochSeconds: z.number().nullable(),
});

export const userSettingsResponseSchema = z.object({
  aiAgents: aiAgentsSettingsSchema,
  intervals: intervalsSettingsSchema,
  options: analysisOptionsSettingsSchema,
  cycling: cyclingSettingsDataSchema,
});

export type UserSettingsResponse = z.infer<typeof userSettingsResponseSchema>;

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

export const updateCyclingRequestSchema = z.object({
  fullName: z.string().nullable().optional(),
  age: z.number().int().positive().max(120).nullable().optional(),
  heightCm: z.number().int().positive().max(300).nullable().optional(),
  weightKg: z.number().positive().max(500).nullable().optional(),
  ftpWatts: z.number().int().positive().max(2500).nullable().optional(),
  hrMaxBpm: z.number().int().positive().max(300).nullable().optional(),
  vo2Max: z.number().positive().max(100).nullable().optional(),
});

export type UpdateCyclingRequest = z.infer<typeof updateCyclingRequestSchema>;

export type CyclingSettingsData = z.infer<typeof cyclingSettingsDataSchema>;
