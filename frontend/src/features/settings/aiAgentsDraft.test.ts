import { describe, expect, it } from 'vitest';

import {
  applyRequestToPersisted,
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

describe('applyRequestToPersisted', () => {
  it('applies provider/model changes to the persisted snapshot', () => {
    const persisted = emptyPersisted();

    const result = applyRequestToPersisted(persisted, {
      selectedProvider: 'gemini',
      selectedModel: 'gemini-3-flash-preview',
    });

    expect(result.selectedProvider).toBe('gemini');
    expect(result.selectedModel).toBe('gemini-3-flash-preview');
    expect(result.workoutChatProvider).toBe('');
  });

  it('applies explicit clears as empty strings', () => {
    const persisted = emptyPersisted();

    const result = applyRequestToPersisted(persisted, {
      selectedProvider: '',
      selectedModel: '',
    });

    expect(result.selectedProvider).toBe('');
    expect(result.selectedModel).toBe('');
  });

  it('applies override fields and includePowerImage', () => {
    const persisted = emptyPersisted();

    const result = applyRequestToPersisted(persisted, {
      workoutChatProvider: 'deepseek',
      workoutChatModel: 'deepseek-v4-flash',
      includePowerImage: true,
    });

    expect(result.workoutChatProvider).toBe('deepseek');
    expect(result.workoutChatModel).toBe('deepseek-v4-flash');
    expect(result.includePowerImage).toBe(true);
    expect(result.selectedProvider).toBe('openai');
  });

  it('ignores API key fields (keys never live in the persisted snapshot)', () => {
    const persisted = emptyPersisted();

    const result = applyRequestToPersisted(persisted, { openaiApiKey: 'sk-abc' });

    expect(result.openaiApiKey).toBe('');
    expect(result.selectedProvider).toBe('openai');
  });
});
