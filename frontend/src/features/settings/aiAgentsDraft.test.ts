import { describe, expect, it } from 'vitest';

import {
  clearRequestedApiKeys,
  createEmptyAiAgentsDraft,
  type AiAgentsDraftState,
} from './aiAgentsDraft';

function emptyPersisted(): AiAgentsDraftState {
  return createEmptyAiAgentsDraft({
    openaiCompatibleBaseUrl: '',
    selectedProvider: 'openai',
    selectedModel: 'gpt-5',
    workoutChatProvider: '',
    workoutChatModel: '',
    workoutPlanningProvider: '',
    workoutPlanningModel: '',
    mesoCycleProvider: '',
    mesoCycleModel: '',
    includePowerImage: false,
  });
}

describe('clearRequestedApiKeys', () => {
  it('clears only the key fields included in the request', () => {
    const draft = {
      ...emptyPersisted(),
      openaiApiKey: 'sk-typed',
      openrouterApiKey: 'or-typed',
    };

    const result = clearRequestedApiKeys(draft, { openaiApiKey: 'sk-typed' });

    expect(result.openaiApiKey).toBe('');
    expect(result.openrouterApiKey).toBe('or-typed');
    expect(result.selectedProvider).toBe('openai');
  });

  it('leaves the draft untouched when no keys are in the request', () => {
    const draft = { ...emptyPersisted(), deepseekApiKey: 'sk-ds' };

    const result = clearRequestedApiKeys(draft, { selectedModel: 'gpt-5.4' });

    expect(result.deepseekApiKey).toBe('sk-ds');
    expect(result).toEqual(draft);
  });
});
