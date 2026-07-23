type AiAgentsFieldKey =
  | 'openaiApiKey'
  | 'geminiApiKey'
  | 'openrouterApiKey'
  | 'deepseekApiKey'
  | 'zaiApiKey'
  | 'openaiCompatibleApiKey'
  | 'openaiCompatibleBaseUrl'
  | 'selectedProvider'
  | 'selectedModel';

type AgentOverrideFieldKey =
  | 'workoutChatProvider'
  | 'workoutChatModel'
  | 'workoutPlanningProvider'
  | 'workoutPlanningModel'
  | 'mesoCycleProvider'
  | 'mesoCycleModel';

type ValidatedAiAgents = Partial<Record<AiAgentsFieldKey | AgentOverrideFieldKey, string | null>>;

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

const AGENT_OVERRIDE_FIELDS: AgentOverrideFieldKey[] = [
  'workoutChatProvider',
  'workoutChatModel',
  'workoutPlanningProvider',
  'workoutPlanningModel',
  'mesoCycleProvider',
  'mesoCycleModel',
];

export function buildAiAgentsConnectionBody(
  data: unknown,
  validated: ValidatedAiAgents,
  options?: { includeAgentOverrides?: boolean },
): Record<string, string | null | boolean> {
  const body: Record<string, string | null | boolean> = {};
  const fields: AiAgentsFieldKey[] = [
    'openaiApiKey',
    'geminiApiKey',
    'openrouterApiKey',
    'deepseekApiKey',
    'zaiApiKey',
    'openaiCompatibleApiKey',
    'openaiCompatibleBaseUrl',
    'selectedProvider',
    'selectedModel',
  ];

  for (const field of fields) {
    const value = getAiAgentsFieldValue(data, field, validated[field]);
    if (value !== undefined) {
      body[field] = value;
    }
  }

  if (options?.includeAgentOverrides) {
    for (const field of AGENT_OVERRIDE_FIELDS) {
      const value = getOptionalStringFieldValue(data, field, validated[field]);
      if (value !== undefined) {
        body[field] = value;
      }
    }
  }

  if (data && typeof data === 'object' && 'includePowerImage' in data) {
    const includePowerImage = (data as Record<string, unknown>).includePowerImage;
    if (typeof includePowerImage === 'boolean') {
      body.includePowerImage = includePowerImage;
    }
  }

  return body;
}
