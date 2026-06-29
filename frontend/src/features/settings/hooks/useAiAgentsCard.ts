import { useEffect, useMemo, useRef, useState } from 'react';

import { testAiAgentsConnection, updateAiAgents } from '../api/settings';
import {
  buildTestStatusMessage,
  buildVisibleAiAgentsRequest,
  clearDraftApiKeys,
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

export function useAiAgentsCard({ settings, apiBaseUrl, onSave }: UseAiAgentsCardOptions) {
  const aiAgents = settings.aiAgents;
  const persistedDraft = useMemo(
    () =>
      createEmptyAiAgentsDraft({
        selectedProvider: aiAgents.selectedProvider ?? '',
        selectedModel: aiAgents.selectedModel ?? '',
        mesoCycleProvider: aiAgents.mesoCycleProvider ?? '',
        mesoCycleModel: aiAgents.mesoCycleModel ?? '',
      }),
    [
      aiAgents.mesoCycleModel,
      aiAgents.mesoCycleProvider,
      aiAgents.selectedModel,
      aiAgents.selectedProvider,
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
  const mesoProviderOption = getProviderOption(draft.mesoCycleProvider);
  const providerKeyState = getProviderKeyState(draft.selectedProvider, draft, aiAgents);
  const validationMessage =
    resolveProviderValidationMessage(draft, providerKeyState) ??
    resolveMesoValidationMessage(draft);
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

  const updateMesoProvider = (value: string) => {
    clearTestStatus();
    setDraft((current) => updateProviderDraft(current, 'mesoCycleProvider', 'mesoCycleModel', value));
  };

  const handleSave = async () => {
    if (!canSave) {
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
    mesoProviderOption,
    updateDraft,
    updateProvider,
    updateMesoProvider,
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
    Boolean(aiAgents.selectedProvider) ||
    Boolean(aiAgents.selectedModel)
  );
}

function resolveMesoValidationMessage(draft: AiAgentsDraftState) {
  const hasProvider = Boolean(draft.mesoCycleProvider.trim());
  const hasModel = Boolean(draft.mesoCycleModel.trim());
  if (hasProvider && !hasModel) {
    return 'Choose a model for the meso cycle provider.';
  }
  if (hasModel && !hasProvider) {
    return 'Choose a provider for the meso cycle model.';
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
  const hasMatchingProviderKey =
    providerKeyState.draftValue.length > 0 || providerKeyState.hasPersistedKey;
  if (draft.selectedProvider && draft.selectedModel.trim() && !hasMatchingProviderKey) {
    return `Add a ${providerKeyState.label} API key or keep the saved one before testing or saving this provider.`;
  }
  return null;
}

function updateProviderDraft(
  current: AiAgentsDraftState,
  providerField: 'selectedProvider' | 'mesoCycleProvider',
  modelField: 'selectedModel' | 'mesoCycleModel',
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
