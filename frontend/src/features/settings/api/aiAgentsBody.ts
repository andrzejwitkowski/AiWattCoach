type AiAgentsFieldKey =
  | 'openaiApiKey'
  | 'geminiApiKey'
  | 'openrouterApiKey'
  | 'deepseekApiKey'
  | 'zaiApiKey'
  | 'selectedProvider'
  | 'selectedModel';

type OptionalProviderOverrideField =
  | 'workoutChatProvider'
  | 'workoutChatModel'
  | 'workoutPlanningProvider'
  | 'workoutPlanningModel'
  | 'mesoCycleProvider'
  | 'mesoCycleModel';

type ValidatedAiAgents = Partial<Record<AiAgentsFieldKey | OptionalProviderOverrideField, string | null>>;

const OPTIONAL_PROVIDER_OVERRIDE_FIELDS: OptionalProviderOverrideField[] = [
  'workoutChatProvider',
  'workoutChatModel',
  'workoutPlanningProvider',
  'workoutPlanningModel',
  'mesoCycleProvider',
  'mesoCycleModel',
];

function trimToUndefined(value: string | null | undefined) {
  const trimmed = value?.trim();
  return trimmed ? trimmed : undefined;
}

function getAiAgentsFieldValue(
  data: unknown,
  key: AiAgentsFieldKey,
  validatedValue: string | null | undefined,
) {
  if (!data || typeof data !== 'object' || !(key in data)) {
    return undefined;
  }

  const rawValue = (data as Record<string, unknown>)[key];

  if (rawValue === null || rawValue === '') {
    return null;
  }

  if (typeof rawValue === 'string' && rawValue.trim() === '') {
    return undefined;
  }

  return trimToUndefined(validatedValue);
}

export function getOptionalStringFieldValue(
  data: unknown,
  key: string,
  validatedValue: string | null | undefined,
) {
  if (!data || typeof data !== 'object' || !(key in data)) {
    return undefined;
  }

  const rawValue = (data as Record<string, unknown>)[key];

  if (rawValue === null) {
    return null;
  }

  if (typeof rawValue === 'string' && rawValue.trim() === '') {
    return null;
  }

  return trimToUndefined(validatedValue);
}

export function buildAiAgentsConnectionBody(
  data: unknown,
  validated: ValidatedAiAgents,
  options?: { includeProviderOverrides?: boolean },
): Record<string, string | null> {
  const body: Record<string, string | null> = {};
  const fields: AiAgentsFieldKey[] = [
    'openaiApiKey',
    'geminiApiKey',
    'openrouterApiKey',
    'deepseekApiKey',
    'zaiApiKey',
    'selectedProvider',
    'selectedModel',
  ];

  for (const field of fields) {
    const value = getAiAgentsFieldValue(data, field, validated[field]);
    if (value !== undefined) {
      body[field] = value;
    }
  }

  if (options?.includeProviderOverrides) {
    for (const field of OPTIONAL_PROVIDER_OVERRIDE_FIELDS) {
      const value = getOptionalStringFieldValue(data, field, validated[field]);
      if (value !== undefined) {
        body[field] = value;
      }
    }
  }

  return body;
}
