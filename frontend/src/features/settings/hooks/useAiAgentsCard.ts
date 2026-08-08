import { useEffect, useMemo, useRef, useState } from 'react';

import { testAiAgentsConnection, updateAiAgents } from '../api/settings';
import {
  buildTestStatusMessage,
  buildVisibleAiAgentsRequest,
  clearRequestedApiKeys,
  createEmptyAiAgentsDraft,
  isAiAgentsDraftDirty,
  mergeDraftWithPersisted,
  type AiAgentsDraftState,
} from '../aiAgentsDraft';
import { getProviderKeyState, getProviderOption } from '../llmProviders';
import { connectionErrorStatus } from '../components/settingsCardFlow';
import type { SettingsStatus } from '../components/SettingsStatusBanner';
import type { TestAiAgentsConnectionResponse, UserSettingsResponse } from '../types';

type UseAiAgentsCardOptions = {
  settings: UserSettingsResponse;
  apiBaseUrl: string;
  onSave: () => void;
};

type OptionalOverrideField =
  | 'workoutChatProvider'
  | 'workoutChatModel'
  | 'workoutPlanningProvider'
  | 'workoutPlanningModel'
  | 'mesoCycleProvider'
  | 'mesoCycleModel';

const OPTIONAL_OVERRIDE_CHECKS: Array<{
  provider: OptionalOverrideField;
  model: OptionalOverrideField;
  label: string;
}> = [
  { provider: 'workoutChatProvider', model: 'workoutChatModel', label: 'post-workout conversation' },
  { provider: 'workoutPlanningProvider', model: 'workoutPlanningModel', label: 'post-workout planning' },
  { provider: 'mesoCycleProvider', model: 'mesoCycleModel', label: 'meso cycle' },
];

export function useAiAgentsCard({ settings, apiBaseUrl, onSave }: UseAiAgentsCardOptions) {
  const aiAgents = settings.aiAgents;
  const persistedDraft = useMemo(
    () =>
      createEmptyAiAgentsDraft({
        openaiCompatibleBaseUrl: aiAgents.openaiCompatibleBaseUrl ?? '',
        selectedProvider: aiAgents.selectedProvider ?? '',
        selectedModel: aiAgents.selectedModel ?? '',
        workoutChatProvider: aiAgents.workoutChatProvider ?? '',
        workoutChatModel: aiAgents.workoutChatModel ?? '',
        workoutPlanningProvider: aiAgents.workoutPlanningProvider ?? '',
        workoutPlanningModel: aiAgents.workoutPlanningModel ?? '',
        mesoCycleProvider: aiAgents.mesoCycleProvider ?? '',
        mesoCycleModel: aiAgents.mesoCycleModel ?? '',
        includePowerImage: aiAgents.includePowerImage ?? false,
      }),
    [
      aiAgents.includePowerImage,
      aiAgents.mesoCycleModel,
      aiAgents.mesoCycleProvider,
      aiAgents.openaiCompatibleBaseUrl,
      aiAgents.selectedModel,
      aiAgents.selectedProvider,
      aiAgents.workoutChatModel,
      aiAgents.workoutChatProvider,
      aiAgents.workoutPlanningModel,
      aiAgents.workoutPlanningProvider,
    ],
  );
  const [draft, setDraft] = useState<AiAgentsDraftState>(persistedDraft);
  const [cleanDraft, setCleanDraft] = useState<AiAgentsDraftState>(persistedDraft);
  const [isSaving, setIsSaving] = useState(false);
  const [isTesting, setIsTesting] = useState(false);
  const [status, setStatus] = useState<SettingsStatus | null>(null);
  const previousPersistedRef = useRef(persistedDraft);
  const testRunIdRef = useRef(0);

  useEffect(() => {
    const previousPersisted = previousPersistedRef.current;
    setDraft((current) => mergeDraftWithPersisted(current, previousPersisted, persistedDraft));
    setCleanDraft((current) => mergeDraftWithPersisted(current, previousPersisted, persistedDraft));
    previousPersistedRef.current = persistedDraft;
  }, [persistedDraft]);

  const hasDirtyDraft = isAiAgentsDraftDirty(draft, cleanDraft);
  const visibleRequest = useMemo(
    () => buildVisibleAiAgentsRequest(draft, persistedDraft),
    [draft, persistedDraft],
  );

  const selectedProviderOption = getProviderOption(draft.selectedProvider);
  const workoutChatProviderOption = getProviderOption(draft.workoutChatProvider);
  const workoutPlanningProviderOption = getProviderOption(draft.workoutPlanningProvider);
  const mesoProviderOption = getProviderOption(draft.mesoCycleProvider);
  const providerKeyState = getProviderKeyState(draft.selectedProvider, draft, aiAgents);
  const validationMessage =
    resolveProviderValidationMessage(draft, providerKeyState) ??
    OPTIONAL_OVERRIDE_CHECKS.map(({ provider, model, label }) =>
      resolveOptionalOverrideValidationMessage(draft[provider], draft[model], label),
    ).find(Boolean) ??
    null;
  const canSave = hasDirtyDraft && !validationMessage;
  const canTest =
    !validationMessage &&
    Boolean(draft.selectedProvider.trim()) &&
    Boolean(draft.selectedModel.trim()) &&
    (Object.keys(visibleRequest).length > 0 || hasAnyPersistedConnectionValue(aiAgents));

  const clearTestStatus = () => {
    testRunIdRef.current += 1;
    setIsTesting(false);
    setStatus(null);
  };

  const updateDraft = (field: keyof AiAgentsDraftState, value: string) => {
    clearTestStatus();
    setDraft((current) => ({ ...current, [field]: value }));
  };

  const updateProvider = (value: string) => {
    clearTestStatus();
    setDraft((current) => updateProviderDraft(current, 'selectedProvider', 'selectedModel', value));
  };

  const updateOverrideProvider = (
    providerField: OptionalOverrideField,
    modelField: OptionalOverrideField,
    value: string,
  ) => {
    clearTestStatus();
    setDraft((current) => updateProviderDraft(current, providerField, modelField, value));
  };

  const toggleIncludePowerImage = (value: boolean) => {
    clearTestStatus();
    setDraft((current) => ({ ...current, includePowerImage: value }));
  };

  const handleSave = async () => {
    if (!canSave) {
      return;
    }

    // The click-time draft is what this request actually persists; use it as
    // the clean baseline so edits made while the save is in flight stay dirty.
    const submittedDraft = draft;
    const submittedCleared = clearRequestedApiKeys(submittedDraft, visibleRequest);

    setIsSaving(true);
    setStatus({
      tone: 'neutral',
      label: 'Saving',
      message: 'Saving current AI provider settings...',
    });

    try {
      await updateAiAgents(apiBaseUrl, visibleRequest);
      setDraft((current) => clearRequestedApiKeys(current, visibleRequest, submittedDraft));
      setCleanDraft(submittedCleared);
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
    if (!canTest) {
      return;
    }

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
      if (testRunId !== testRunIdRef.current) {
        return;
      }
      setStatusFromTest(result);
    } catch (err) {
      if (testRunId !== testRunIdRef.current) {
        return;
      }
      setStatus(connectionErrorStatus(err, 'Failed to test AI provider connection'));
    } finally {
      if (testRunId === testRunIdRef.current) {
        setIsTesting(false);
      }
    }
  };

  function setStatusFromTest(result: TestAiAgentsConnectionResponse) {
    setStatus({
      tone: result.connected ? 'success' : 'error',
      label: result.connected ? 'OK' : 'FAILED',
      message: buildTestStatusMessage(result),
    });
  }

  return {
    aiAgents,
    draft,
    status,
    isSaving,
    isTesting,
    canSave,
    canTest,
    hasDirtyDraft,
    validationMessage,
    selectedProviderOption,
    workoutChatProviderOption,
    workoutPlanningProviderOption,
    mesoProviderOption,
    updateDraft,
    updateProvider,
    updateOverrideProvider,
    toggleIncludePowerImage,
    handleSave,
    handleTest,
  };
}

function hasAnyPersistedConnectionValue(aiAgents: UserSettingsResponse['aiAgents']) {
  return (
    aiAgents.openaiApiKeySet ||
    aiAgents.geminiApiKeySet ||
    aiAgents.openrouterApiKeySet ||
    aiAgents.deepseekApiKeySet ||
    aiAgents.zaiApiKeySet ||
    aiAgents.openaiCompatibleApiKeySet ||
    Boolean(aiAgents.openaiCompatibleBaseUrl) ||
    Boolean(aiAgents.selectedProvider) ||
    Boolean(aiAgents.selectedModel)
  );
}

function resolveOptionalOverrideValidationMessage(
  provider: string,
  model: string,
  label: string,
) {
  const hasProvider = Boolean(provider.trim());
  const hasModel = Boolean(model.trim());
  if (hasProvider && !hasModel) {
    return `Choose a model for the ${label} provider.`;
  }
  if (hasModel && !hasProvider) {
    return `Choose a provider for the ${label} model.`;
  }
  return null;
}

function resolveProviderValidationMessage(
  draft: AiAgentsDraftState,
  providerKeyState: ReturnType<typeof getProviderKeyState>,
) {
  if (draft.selectedProvider && !draft.selectedModel.trim()) {
    return 'Choose a model for the selected provider.';
  }
  if (draft.selectedModel.trim() && !draft.selectedProvider) {
    return 'Choose a provider for the selected model.';
  }
  const usesOpenaiCompatible = [
    draft.selectedProvider,
    draft.workoutChatProvider,
    draft.workoutPlanningProvider,
    draft.mesoCycleProvider,
  ].some((provider) => provider === 'openai_compatible');
  if (usesOpenaiCompatible) {
    const baseUrl = draft.openaiCompatibleBaseUrl.trim();
    if (!baseUrl) {
      return 'Add a base URL for the OpenAI Compatible provider.';
    }
    if (!isAbsoluteHttpUrl(baseUrl)) {
      return 'OpenAI Compatible base URL must be an absolute http(s) URL.';
    }
  }
  const hasMatchingProviderKey =
    providerKeyState.draftValue.length > 0 || providerKeyState.hasPersistedKey;
  if (draft.selectedProvider && draft.selectedModel.trim() && !hasMatchingProviderKey) {
    return `Add a ${providerKeyState.label} API key or keep the saved one before testing or saving this provider.`;
  }
  return null;
}

function isAbsoluteHttpUrl(value: string) {
  try {
    const parsed = new URL(value);
    return parsed.protocol === 'http:' || parsed.protocol === 'https:';
  } catch {
    return false;
  }
}

function updateProviderDraft(
  current: AiAgentsDraftState,
  providerField: 'selectedProvider' | OptionalOverrideField,
  modelField: 'selectedModel' | OptionalOverrideField,
  value: string,
): AiAgentsDraftState {
  const previousOption = getProviderOption(current[providerField]);
  const nextOption = getProviderOption(value);
  const currentModel = current[modelField].trim();
  const shouldAutofillModel =
    Boolean(nextOption) && (!currentModel || previousOption?.suggestedModels.includes(currentModel));

  return {
    ...current,
    [providerField]: value,
    [modelField]: shouldAutofillModel ? nextOption?.suggestedModels[0] ?? current[modelField] : current[modelField],
  };
}
