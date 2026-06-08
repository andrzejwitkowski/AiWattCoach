const TRIMMED_STRING_FIELDS = [
  'openaiApiKey',
  'geminiApiKey',
  'openrouterApiKey',
  'deepseekApiKey',
  'selectedProvider',
  'selectedModel',
  'mesoCycleProvider',
  'mesoCycleModel',
] as const;

export function normalizeAiAgentsPayload(data: unknown) {
  if (!data || typeof data !== 'object') {
    return data;
  }

  const candidate = data as Record<string, unknown>;
  const normalized: Record<string, unknown> = { ...candidate };

  for (const field of TRIMMED_STRING_FIELDS) {
    const value = candidate[field];
    if (typeof value === 'string') {
      normalized[field] = value.trim();
    }
  }

  return normalized;
}
