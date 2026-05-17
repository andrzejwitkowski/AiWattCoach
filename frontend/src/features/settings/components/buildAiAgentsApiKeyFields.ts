import type { UserSettingsResponse } from '../types';
import type { ApiKeyFieldConfig } from './AiAgentsCardSections';
import type { DraftState } from './AiAgentsCard.shared';

type BuildAiAgentsApiKeyFieldsArgs = {
  aiAgents: UserSettingsResponse['aiAgents'];
  draft: DraftState;
  showOpenai: boolean;
  showGemini: boolean;
  showOpenrouter: boolean;
  showDeepseek: boolean;
  openaiHasKey: boolean;
  geminiHasKey: boolean;
  openrouterHasKey: boolean;
  deepseekHasKey: boolean;
  setShowOpenai: (updater: (value: boolean) => boolean) => void;
  setShowGemini: (updater: (value: boolean) => boolean) => void;
  setShowOpenrouter: (updater: (value: boolean) => boolean) => void;
  setShowDeepseek: (updater: (value: boolean) => boolean) => void;
  updateDraft: (field: keyof DraftState, value: string) => void;
};

export function buildAiAgentsApiKeyFields({
  aiAgents,
  draft,
  showOpenai,
  showGemini,
  showOpenrouter,
  showDeepseek,
  openaiHasKey,
  geminiHasKey,
  openrouterHasKey,
  deepseekHasKey,
  setShowOpenai,
  setShowGemini,
  setShowOpenrouter,
  setShowDeepseek,
  updateDraft,
}: BuildAiAgentsApiKeyFieldsArgs): ApiKeyFieldConfig[] {
  return [
    {
      id: 'openai-api-key',
      label: 'OpenAI API Key',
      placeholder: aiAgents.openaiApiKeySet ? 'Already configured' : 'sk-...',
      value: draft.openaiApiKey,
      visible: showOpenai,
      configured: aiAgents.openaiApiKeySet,
      emphasized: !draft.selectedProvider || draft.selectedProvider === 'openai',
      helperText:
        draft.selectedProvider === 'openai'
          ? 'Used by the active provider.'
          : openaiHasKey
            ? 'Saved for quick provider switching.'
            : 'Optional unless you switch to this provider.',
      onVisibilityChange: () => setShowOpenai((value) => !value),
      onChange: (value) => updateDraft('openaiApiKey', value),
    },
    {
      id: 'gemini-api-key',
      label: 'Gemini API Key',
      placeholder: aiAgents.geminiApiKeySet ? 'Already configured' : 'AIza...',
      value: draft.geminiApiKey,
      visible: showGemini,
      configured: aiAgents.geminiApiKeySet,
      emphasized: !draft.selectedProvider || draft.selectedProvider === 'gemini',
      helperText:
        draft.selectedProvider === 'gemini'
          ? 'Used by the active provider.'
          : geminiHasKey
            ? 'Saved for quick provider switching.'
            : 'Optional unless you switch to this provider.',
      onVisibilityChange: () => setShowGemini((value) => !value),
      onChange: (value) => updateDraft('geminiApiKey', value),
    },
    {
      id: 'openrouter-api-key',
      label: 'OpenRouter API Key',
      placeholder: aiAgents.openrouterApiKeySet ? 'Already configured' : 'sk-or-...',
      value: draft.openrouterApiKey,
      visible: showOpenrouter,
      configured: aiAgents.openrouterApiKeySet,
      emphasized: !draft.selectedProvider || draft.selectedProvider === 'openrouter',
      helperText:
        draft.selectedProvider === 'openrouter'
          ? 'Used by the active provider.'
          : openrouterHasKey
            ? 'Saved for quick provider switching.'
            : 'Optional unless you switch to this provider.',
      onVisibilityChange: () => setShowOpenrouter((value) => !value),
      onChange: (value) => updateDraft('openrouterApiKey', value),
    },
    {
      id: 'deepseek-api-key',
      label: 'DeepSeek API Key',
      placeholder: aiAgents.deepseekApiKeySet ? 'Already configured' : 'sk-...',
      value: draft.deepseekApiKey,
      visible: showDeepseek,
      configured: aiAgents.deepseekApiKeySet,
      emphasized: !draft.selectedProvider || draft.selectedProvider === 'deepseek',
      helperText:
        draft.selectedProvider === 'deepseek'
          ? 'Used by the active provider.'
          : deepseekHasKey
            ? 'Saved for quick provider switching.'
            : 'Optional unless you switch to this provider.',
      onVisibilityChange: () => setShowDeepseek((value) => !value),
      onChange: (value) => updateDraft('deepseekApiKey', value),
    },
  ];
}
