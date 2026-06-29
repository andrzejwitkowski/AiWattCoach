import type { LlmProvider, UserSettingsResponse } from './types';

export type ProviderOption = {
  value: LlmProvider;
  label: string;
  suggestedModels: string[];
};

export const PROVIDER_OPTIONS: ProviderOption[] = [
  { value: 'openai', label: 'OpenAI', suggestedModels: ['gpt-5', 'gpt-5.4', 'o4-mini'] },
  { value: 'gemini', label: 'Gemini', suggestedModels: ['gemini-3-flash-preview', 'gemini-2.5-flash', 'gemini-2.5-pro'] },
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
  {
    value: 'zai',
    label: 'z.ai',
    suggestedModels: ['glm-5.2'],
  },
];

export function getProviderOption(provider: string) {
  return PROVIDER_OPTIONS.find((option) => option.value === provider);
}

type AiAgentsSettings = UserSettingsResponse['aiAgents'];

export function isLlmProviderKeyConfigured(
  provider: string | null | undefined,
  aiAgents: AiAgentsSettings,
): boolean {
  switch (provider) {
    case 'openai':
      return aiAgents.openaiApiKeySet;
    case 'gemini':
      return aiAgents.geminiApiKeySet;
    case 'openrouter':
      return aiAgents.openrouterApiKeySet;
    case 'deepseek':
      return aiAgents.deepseekApiKeySet;
    case 'zai':
      return aiAgents.zaiApiKeySet;
    default:
      return false;
  }
}

type ProviderDraftKeys = {
  openaiApiKey: string;
  geminiApiKey: string;
  openrouterApiKey: string;
  deepseekApiKey: string;
  zaiApiKey: string;
};

export function getProviderKeyState(
  provider: string,
  draft: ProviderDraftKeys,
  aiAgents: AiAgentsSettings,
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
    case 'zai':
      return {
        draftValue: draft.zaiApiKey.trim(),
        hasPersistedKey: aiAgents.zaiApiKeySet,
        label: 'z.ai',
      };
    default:
      return { draftValue: '', hasPersistedKey: false, label: 'Provider' };
  }
}
