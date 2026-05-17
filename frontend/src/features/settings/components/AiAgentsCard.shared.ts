import type { LlmProvider, TestAiAgentsConnectionResponse, UserSettingsResponse } from '../types';

export type DraftState = {
  openaiApiKey: string;
  geminiApiKey: string;
  openrouterApiKey: string;
  deepseekApiKey: string;
  selectedProvider: string;
  selectedModel: string;
  trainingPlanSupervisorModel: string;
  trainingPlanSupervisorEnabled: boolean;
};

export type AiAgentsCardStatus = {
  tone: 'neutral' | 'success' | 'error';
  label: string;
  message: string;
};

type ProviderOption = {
  value: LlmProvider;
  label: string;
  suggestedModels: string[];
};

export const DEFAULT_TRAINING_PLAN_SUPERVISOR_MODEL = 'gemini-2.5-pro';

export const PROVIDER_OPTIONS: ProviderOption[] = [
  { value: 'openai', label: 'OpenAI', suggestedModels: ['gpt-5', 'gpt-5.4', 'o4-mini'] },
  {
    value: 'gemini',
    label: 'Gemini',
    suggestedModels: ['gemini-3-flash-preview', 'gemini-2.5-flash', 'gemini-2.5-pro'],
  },
  {
    value: 'openrouter',
    label: 'OpenRouter',
    suggestedModels: ['openai/gpt-5', 'google/gemini-3-flash-preview', 'anthropic/claude-sonnet-4.5'],
  },
  {
    value: 'deepseek',
    label: 'DeepSeek',
    suggestedModels: ['deepseek-v4-flash', 'deepseek-v4-pro'],
  },
];

export const SUPERVISOR_MODELS = [
  DEFAULT_TRAINING_PLAN_SUPERVISOR_MODEL,
  'gemini-2.5-flash',
  'gemini-3-flash-preview',
];

export function clearDraftApiKeys(draft: DraftState): DraftState {
  return {
    ...draft,
    openaiApiKey: '',
    geminiApiKey: '',
    openrouterApiKey: '',
    deepseekApiKey: '',
  };
}

export function getProviderOption(provider: string) {
  return PROVIDER_OPTIONS.find((option) => option.value === provider);
}

export function getProviderKeyState(
  provider: string,
  draft: DraftState,
  aiAgents: UserSettingsResponse['aiAgents'],
) {
  switch (provider) {
    case 'openai':
      return {
        draftValue: draft.openaiApiKey.trim(),
        hasPersistedKey: aiAgents.openaiApiKeySet,
        label: 'OpenAI',
      };
    case 'gemini':
      return {
        draftValue: draft.geminiApiKey.trim(),
        hasPersistedKey: aiAgents.geminiApiKeySet,
        label: 'Gemini',
      };
    case 'openrouter':
      return {
        draftValue: draft.openrouterApiKey.trim(),
        hasPersistedKey: aiAgents.openrouterApiKeySet,
        label: 'OpenRouter',
      };
    case 'deepseek':
      return {
        draftValue: draft.deepseekApiKey.trim(),
        hasPersistedKey: aiAgents.deepseekApiKeySet,
        label: 'DeepSeek',
      };
    default:
      return {
        draftValue: '',
        hasPersistedKey: false,
        label: 'Provider',
      };
  }
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
