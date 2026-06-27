import type { TestAiAgentsConnectionResponse } from './types';

export type AiAgentsDraftState = {
  openaiApiKey: string;
  geminiApiKey: string;
  openrouterApiKey: string;
  deepseekApiKey: string;
  zaiApiKey: string;
  selectedProvider: string;
  selectedModel: string;
  mesoCycleProvider: string;
  mesoCycleModel: string;
};

export function createEmptyAiAgentsDraft(
  persisted: Pick<
    AiAgentsDraftState,
    'selectedProvider' | 'selectedModel' | 'mesoCycleProvider' | 'mesoCycleModel'
  >,
): AiAgentsDraftState {
  return {
    openaiApiKey: '',
    geminiApiKey: '',
    openrouterApiKey: '',
    deepseekApiKey: '',
    zaiApiKey: '',
    selectedProvider: persisted.selectedProvider,
    selectedModel: persisted.selectedModel,
    mesoCycleProvider: persisted.mesoCycleProvider,
    mesoCycleModel: persisted.mesoCycleModel,
  };
}

export function clearDraftApiKeys(draft: AiAgentsDraftState): AiAgentsDraftState {
  return {
    ...draft,
    openaiApiKey: '',
    geminiApiKey: '',
    openrouterApiKey: '',
    deepseekApiKey: '',
    zaiApiKey: '',
  };
}

export function mergeDraftWithPersisted(
  current: AiAgentsDraftState,
  previousPersisted: AiAgentsDraftState,
  persisted: AiAgentsDraftState,
): AiAgentsDraftState {
  return {
    openaiApiKey:
      current.openaiApiKey === previousPersisted.openaiApiKey ? persisted.openaiApiKey : current.openaiApiKey,
    geminiApiKey:
      current.geminiApiKey === previousPersisted.geminiApiKey ? persisted.geminiApiKey : current.geminiApiKey,
    openrouterApiKey:
      current.openrouterApiKey === previousPersisted.openrouterApiKey
        ? persisted.openrouterApiKey
        : current.openrouterApiKey,
    deepseekApiKey:
      current.deepseekApiKey === previousPersisted.deepseekApiKey
        ? persisted.deepseekApiKey
        : current.deepseekApiKey,
    zaiApiKey:
      current.zaiApiKey === previousPersisted.zaiApiKey ? persisted.zaiApiKey : current.zaiApiKey,
    selectedProvider:
      current.selectedProvider === previousPersisted.selectedProvider
        ? persisted.selectedProvider
        : current.selectedProvider,
    selectedModel:
      current.selectedModel === previousPersisted.selectedModel ? persisted.selectedModel : current.selectedModel,
    mesoCycleProvider:
      current.mesoCycleProvider === previousPersisted.mesoCycleProvider
        ? persisted.mesoCycleProvider
        : current.mesoCycleProvider,
    mesoCycleModel:
      current.mesoCycleModel === previousPersisted.mesoCycleModel
        ? persisted.mesoCycleModel
        : current.mesoCycleModel,
  };
}

export function isAiAgentsDraftDirty(current: AiAgentsDraftState, clean: AiAgentsDraftState): boolean {
  return (
    current.openaiApiKey !== clean.openaiApiKey ||
    current.geminiApiKey !== clean.geminiApiKey ||
    current.openrouterApiKey !== clean.openrouterApiKey ||
    current.deepseekApiKey !== clean.deepseekApiKey ||
    current.zaiApiKey !== clean.zaiApiKey ||
    current.selectedProvider !== clean.selectedProvider ||
    current.selectedModel !== clean.selectedModel ||
    current.mesoCycleProvider !== clean.mesoCycleProvider ||
    current.mesoCycleModel !== clean.mesoCycleModel
  );
}

function assignTrimmedApiKey(
  request: Partial<Record<keyof AiAgentsDraftState, string | null>>,
  key: keyof AiAgentsDraftState,
  value: string,
) {
  const trimmed = value.trim();
  if (trimmed) {
    request[key] = trimmed;
  }
}

function assignChangedStringField(
  request: Partial<Record<keyof AiAgentsDraftState, string | null>>,
  key: keyof AiAgentsDraftState,
  currentValue: string,
  persistedValue: string,
) {
  const trimmed = currentValue.trim();
  if (trimmed !== persistedValue) {
    request[key] = trimmed.length > 0 ? trimmed : '';
  }
}

export function buildVisibleAiAgentsRequest(
  draft: AiAgentsDraftState,
  persisted: AiAgentsDraftState,
): Partial<Record<keyof AiAgentsDraftState, string | null>> {
  const request: Partial<Record<keyof AiAgentsDraftState, string | null>> = {};

  assignTrimmedApiKey(request, 'openaiApiKey', draft.openaiApiKey);
  assignTrimmedApiKey(request, 'geminiApiKey', draft.geminiApiKey);
  assignTrimmedApiKey(request, 'openrouterApiKey', draft.openrouterApiKey);
  assignTrimmedApiKey(request, 'deepseekApiKey', draft.deepseekApiKey);
  assignTrimmedApiKey(request, 'zaiApiKey', draft.zaiApiKey);

  const trimmedProvider = draft.selectedProvider.trim();
  const trimmedModel = draft.selectedModel.trim();
  if (trimmedProvider !== persisted.selectedProvider) {
    request.selectedProvider = trimmedProvider.length > 0 ? trimmedProvider : '';
    if (trimmedModel.length > 0) {
      request.selectedModel = trimmedModel;
    }
  }
  if (trimmedModel !== persisted.selectedModel && !('selectedModel' in request)) {
    request.selectedModel = trimmedModel.length > 0 ? trimmedModel : '';
  }

  assignChangedStringField(request, 'mesoCycleProvider', draft.mesoCycleProvider, persisted.mesoCycleProvider);
  assignChangedStringField(request, 'mesoCycleModel', draft.mesoCycleModel, persisted.mesoCycleModel);

  return request;
}

export function buildTestStatusMessage(result: TestAiAgentsConnectionResponse) {
  const reusedSavedValues = [
    result.usedSavedApiKey ? 'saved key' : null,
    result.usedSavedProvider ? 'saved provider' : null,
    result.usedSavedModel ? 'saved model' : null,
  ].filter(Boolean);

  if (reusedSavedValues.length === 0) {
    return `${result.message} Tested the visible draft only.`;
  }

  return `${result.message} Used ${reusedSavedValues.join(', ')} for unchanged fields.`;
}
