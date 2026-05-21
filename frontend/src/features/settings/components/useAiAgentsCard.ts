import { useEffect, useMemo, useRef, useState } from 'react';

import { testAiAgentsConnection, updateAiAgents } from '../api/settings';
import type { TestAiAgentsConnectionResponse, UserSettingsResponse } from '../types';
import {
  buildTestStatusMessage,
  clearDraftApiKeys,
  DEFAULT_TRAINING_PLAN_SUPERVISOR_MODEL,
  type DraftState,
  getProviderKeyState,
  getProviderOption,
  SUPERVISOR_MODELS,
  type AiAgentsCardStatus,
} from './AiAgentsCard.shared';
import { buildAiAgentsApiKeyFields } from './buildAiAgentsApiKeyFields';
import { useAiAgentsDraftSync } from './useAiAgentsDraftSync';

type VisibleRequest = Partial<Record<keyof DraftState, string | boolean | null>>;

type UseAiAgentsCardArgs = {
  settings: UserSettingsResponse;
  apiBaseUrl: string;
  onSave: () => void;
};

function buildVisibleRequest(draft: DraftState, persistedDraft: DraftState): VisibleRequest {
  const request: VisibleRequest = {};
  const trimmedOpenai = draft.openaiApiKey.trim();
  const trimmedGemini = draft.geminiApiKey.trim();
  const trimmedOpenrouter = draft.openrouterApiKey.trim();
  const trimmedDeepseek = draft.deepseekApiKey.trim();
  const trimmedProvider = draft.selectedProvider.trim();
  const trimmedModel = draft.selectedModel.trim();
  const trimmedSupervisorModel = draft.trainingPlanSupervisorModel.trim();

  if (trimmedOpenai) {
    request.openaiApiKey = trimmedOpenai;
  }
  if (trimmedGemini) {
    request.geminiApiKey = trimmedGemini;
  }
  if (trimmedOpenrouter) {
    request.openrouterApiKey = trimmedOpenrouter;
  }
  if (trimmedDeepseek) {
    request.deepseekApiKey = trimmedDeepseek;
  }
  if (trimmedProvider !== persistedDraft.selectedProvider) {
    request.selectedProvider = trimmedProvider.length > 0 ? trimmedProvider : '';
    if (trimmedModel.length > 0) {
      request.selectedModel = trimmedModel;
    }
  }
  if (trimmedModel !== persistedDraft.selectedModel && !('selectedModel' in request)) {
    request.selectedModel = trimmedModel.length > 0 ? trimmedModel : '';
  }
  if (draft.trainingPlanSupervisorEnabled !== persistedDraft.trainingPlanSupervisorEnabled) {
    request.trainingPlanSupervisorEnabled = draft.trainingPlanSupervisorEnabled;
  }
  if (trimmedSupervisorModel !== persistedDraft.trainingPlanSupervisorModel) {
    request.trainingPlanSupervisorModel = trimmedSupervisorModel.length > 0 ? trimmedSupervisorModel : '';
  }

  return request;
}

export function useAiAgentsCard({ settings, apiBaseUrl, onSave }: UseAiAgentsCardArgs) {
  const aiAgents = settings.aiAgents;
  const persistedDraft = useMemo(
    () => ({
      openaiApiKey: '',
      geminiApiKey: '',
      openrouterApiKey: '',
      deepseekApiKey: '',
      selectedProvider: aiAgents.selectedProvider ?? '',
      selectedModel: aiAgents.selectedModel ?? '',
      trainingPlanSupervisorModel:
        aiAgents.trainingPlanSupervisorModel ?? DEFAULT_TRAINING_PLAN_SUPERVISOR_MODEL,
      trainingPlanSupervisorEnabled: aiAgents.trainingPlanSupervisorEnabled,
    }),
    [
      aiAgents.selectedModel,
      aiAgents.selectedProvider,
      aiAgents.trainingPlanSupervisorEnabled,
      aiAgents.trainingPlanSupervisorModel,
    ],
  );
  const { draft, setDraft, cleanDraft, setCleanDraft } = useAiAgentsDraftSync(persistedDraft);
  const [showOpenai, setShowOpenai] = useState(false);
  const [showGemini, setShowGemini] = useState(false);
  const [showOpenrouter, setShowOpenrouter] = useState(false);
  const [showDeepseek, setShowDeepseek] = useState(false);
  const [isSaving, setIsSaving] = useState(false);
  const [isTesting, setIsTesting] = useState(false);
  const [status, setStatus] = useState<AiAgentsCardStatus | null>(null);
  const [showSupervisorGeminiKeyModal, setShowSupervisorGeminiKeyModal] = useState(false);
  const testRunIdRef = useRef(0);

  useEffect(() => {
    if (!showSupervisorGeminiKeyModal) {
      return undefined;
    }

    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        setShowSupervisorGeminiKeyModal(false);
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => {
      window.removeEventListener('keydown', handleKeyDown);
    };
  }, [showSupervisorGeminiKeyModal]);

  const visibleRequest = useMemo(() => buildVisibleRequest(draft, persistedDraft), [draft, persistedDraft]);
  const selectedProviderOption = getProviderOption(draft.selectedProvider);
  const suggestedModels = selectedProviderOption?.suggestedModels ?? [];
  const supervisorModelOptions = useMemo(() => {
    const trimmedValue = draft.trainingPlanSupervisorModel.trim();
    return trimmedValue && !SUPERVISOR_MODELS.includes(trimmedValue)
      ? [trimmedValue, ...SUPERVISOR_MODELS]
      : SUPERVISOR_MODELS;
  }, [draft.trainingPlanSupervisorModel]);

  const hasDirtyDraft =
    draft.openaiApiKey !== cleanDraft.openaiApiKey ||
    draft.geminiApiKey !== cleanDraft.geminiApiKey ||
    draft.openrouterApiKey !== cleanDraft.openrouterApiKey ||
    draft.deepseekApiKey !== cleanDraft.deepseekApiKey ||
    draft.selectedProvider !== cleanDraft.selectedProvider ||
    draft.selectedModel !== cleanDraft.selectedModel ||
    draft.trainingPlanSupervisorModel !== cleanDraft.trainingPlanSupervisorModel ||
    draft.trainingPlanSupervisorEnabled !== cleanDraft.trainingPlanSupervisorEnabled;
  const hasAnyPersistedConnectionValue =
    aiAgents.openaiApiKeySet ||
    aiAgents.geminiApiKeySet ||
    aiAgents.openrouterApiKeySet ||
    aiAgents.deepseekApiKeySet ||
    Boolean(aiAgents.selectedProvider) ||
    Boolean(aiAgents.selectedModel) ||
    aiAgents.trainingPlanSupervisorEnabled ||
    Boolean(aiAgents.trainingPlanSupervisorModel);
  const openaiHasKey = aiAgents.openaiApiKeySet || draft.openaiApiKey.trim().length > 0;
  const geminiHasKey = aiAgents.geminiApiKeySet || draft.geminiApiKey.trim().length > 0;
  const openrouterHasKey = aiAgents.openrouterApiKeySet || draft.openrouterApiKey.trim().length > 0;
  const deepseekHasKey = aiAgents.deepseekApiKeySet || draft.deepseekApiKey.trim().length > 0;
  const providerKeyState = getProviderKeyState(draft.selectedProvider, draft, aiAgents);
  const hasMatchingProviderKey =
    providerKeyState.draftValue.length > 0 || providerKeyState.hasPersistedKey;
  const providerValidationMessage =
    draft.selectedProvider && !draft.selectedModel.trim()
      ? 'Choose a model for the selected provider.'
      : draft.selectedModel.trim() && !draft.selectedProvider
        ? 'Choose a provider for the selected model.'
        : null;
  const providerKeyValidationMessage =
    draft.selectedProvider && draft.selectedModel.trim() && !hasMatchingProviderKey
      ? `Add a ${providerKeyState.label} API key or keep the saved one before testing or saving this provider.`
      : null;
  const validationMessage = providerValidationMessage ?? providerKeyValidationMessage;
  const canSave = hasDirtyDraft && !validationMessage;
  const canTest =
    !validationMessage &&
    Boolean(draft.selectedProvider.trim()) &&
    Boolean(draft.selectedModel.trim()) &&
    (Object.keys(visibleRequest).length > 0 || hasAnyPersistedConnectionValue);

  const clearTestStatusIfNeeded = () => {
    testRunIdRef.current += 1;
    setIsTesting(false);
    setStatus(null);
  };

  const setStatusFromTest = (result: TestAiAgentsConnectionResponse) => {
    setStatus({
      tone: result.connected ? 'success' : 'error',
      label: result.connected ? 'OK' : 'FAILED',
      message: buildTestStatusMessage(result),
    });
  };

  const updateDraft = (field: keyof DraftState, value: string) => {
    if (isSaving) return;
    clearTestStatusIfNeeded();
    setDraft((current) => ({ ...current, [field]: value }));
  };

  const updateSupervisorEnabled = (enabled: boolean) => {
    if (isSaving) return;
    clearTestStatusIfNeeded();
    setDraft((current) => ({
      ...current,
      trainingPlanSupervisorEnabled: enabled,
      trainingPlanSupervisorModel:
        current.trainingPlanSupervisorModel || DEFAULT_TRAINING_PLAN_SUPERVISOR_MODEL,
    }));
  };

  const updateProvider = (value: string) => {
    if (isSaving) return;
    clearTestStatusIfNeeded();
    setDraft((current) => {
      const previousOption = getProviderOption(current.selectedProvider);
      const nextOption = getProviderOption(value);
      const currentModel = current.selectedModel.trim();
      const shouldAutofillModel =
        Boolean(nextOption) &&
        (!currentModel || previousOption?.suggestedModels.includes(currentModel));

      return {
        ...current,
        selectedProvider: value,
        selectedModel: shouldAutofillModel
          ? nextOption?.suggestedModels[0] ?? current.selectedModel
          : current.selectedModel,
      };
    });
  };

  const apiKeyFields = buildAiAgentsApiKeyFields({
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
  });

  const handleSave = async () => {
    if (!canSave) return;
    if (draft.trainingPlanSupervisorEnabled && !geminiHasKey) {
      setShowSupervisorGeminiKeyModal(true);
      return;
    }
    setIsSaving(true);
    setStatus({
      tone: 'neutral',
      label: 'Saving',
      message: 'Saving current AI provider settings...',
    });

    try {
      await updateAiAgents(apiBaseUrl, visibleRequest);
      const clearedDraft = clearDraftApiKeys(draft);
      setDraft(clearedDraft);
      setCleanDraft(clearedDraft);
      setStatus({
        tone: 'success',
        label: 'Saved',
        message: 'AI provider settings saved. New coach replies will use the latest provider setup.',
      });
      onSave();
    } catch (err) {
      setStatus({
        tone: 'error',
        label: 'Save failed',
        message: err instanceof Error ? err.message : 'Failed to save AI settings',
      });
    } finally {
      setIsSaving(false);
    }
  };

  const handleTest = async () => {
    if (!canTest) return;
    const testRunId = testRunIdRef.current + 1;
    testRunIdRef.current = testRunId;
    setIsTesting(true);
    setStatus({
      tone: 'neutral',
      label: 'Testing',
      message: 'Testing the current visible AI draft...',
    });

    try {
      const result = await testAiAgentsConnection(apiBaseUrl, visibleRequest);
      if (testRunId !== testRunIdRef.current) return;
      setStatusFromTest(result);
    } catch (err) {
      if (testRunId !== testRunIdRef.current) return;
      setStatus({
        tone: 'error',
        label: 'FAILED',
        message: err instanceof Error ? err.message : 'Failed to test AI provider connection',
      });
    } finally {
      if (testRunId === testRunIdRef.current) {
        setIsTesting(false);
      }
    }
  };

  return {
    draft,
    suggestedModels,
    supervisorModelOptions,
    validationMessage,
    apiKeyFields,
    status,
    isSaving,
    isTesting,
    canTest,
    canSave,
    hasDirtyDraft,
    showSupervisorGeminiKeyModal,
    setShowSupervisorGeminiKeyModal,
    updateDraft,
    updateProvider,
    updateSupervisorEnabled,
    handleSave,
    handleTest,
  };
}
