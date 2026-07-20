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

type ValidatedAiAgents = Partial<Record<AiAgentsFieldKey | 'mesoCycleProvider' | 'mesoCycleModel', string | null>>;

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
  options?: { includeMesoFields?: boolean },
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

  if (options?.includeMesoFields) {
    const mesoCycleProvider = getOptionalStringFieldValue(data, 'mesoCycleProvider', validated.mesoCycleProvider);
    const mesoCycleModel = getOptionalStringFieldValue(data, 'mesoCycleModel', validated.mesoCycleModel);

    if (mesoCycleProvider !== undefined) {
      body.mesoCycleProvider = mesoCycleProvider;
    }
    if (mesoCycleModel !== undefined) {
      body.mesoCycleModel = mesoCycleModel;
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
