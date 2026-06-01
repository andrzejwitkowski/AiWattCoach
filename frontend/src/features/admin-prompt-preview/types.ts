import { z } from 'zod';

const llmChatMessageSchema = z
  .object({
    role: z.string(),
    content: z.string(),
  })
  .passthrough();

const llmToolDefinitionSchema = z
  .object({
    name: z.string(),
    description: z.string(),
  })
  .passthrough();

export const adminPromptPreviewResponseSchema = z.object({
  meta: z.object({
    userId: z.string(),
    date: z.string(),
    surface: z.string(),
    provider: z.string(),
    model: z.string(),
    focusDate: z.string(),
    selectedWorkoutId: z.string().optional(),
    selectionMethod: z.string().optional(),
    complianceScore: z.number().optional(),
  }),
  request: z.object({
    systemPrompt: z.string(),
    stableContext: z.string(),
    volatileContext: z.string(),
    conversation: z.array(llmChatMessageSchema),
    tools: z.array(llmToolDefinitionSchema),
    toolChoice: z.unknown(),
  }),
  providerMessages: z.array(
    z.object({
      role: z.string(),
      content: z.string(),
    }),
  ),
});

export type AdminPromptPreviewResponse = z.infer<typeof adminPromptPreviewResponseSchema>;
