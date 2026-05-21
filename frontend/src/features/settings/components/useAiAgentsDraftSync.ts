import { useEffect, useRef, useState } from 'react';

import type { DraftState } from './AiAgentsCard.shared';

function mergePersistedDraft(
  current: DraftState,
  previousPersisted: DraftState,
  persistedDraft: DraftState,
): DraftState {
  return {
    openaiApiKey:
      current.openaiApiKey === previousPersisted.openaiApiKey
        ? persistedDraft.openaiApiKey
        : current.openaiApiKey,
    geminiApiKey:
      current.geminiApiKey === previousPersisted.geminiApiKey
        ? persistedDraft.geminiApiKey
        : current.geminiApiKey,
    openrouterApiKey:
      current.openrouterApiKey === previousPersisted.openrouterApiKey
        ? persistedDraft.openrouterApiKey
        : current.openrouterApiKey,
    deepseekApiKey:
      current.deepseekApiKey === previousPersisted.deepseekApiKey
        ? persistedDraft.deepseekApiKey
        : current.deepseekApiKey,
    selectedProvider:
      current.selectedProvider === previousPersisted.selectedProvider
        ? persistedDraft.selectedProvider
        : current.selectedProvider,
    selectedModel:
      current.selectedModel === previousPersisted.selectedModel
        ? persistedDraft.selectedModel
        : current.selectedModel,
    trainingPlanSupervisorModel:
      current.trainingPlanSupervisorModel === previousPersisted.trainingPlanSupervisorModel
        ? persistedDraft.trainingPlanSupervisorModel
        : current.trainingPlanSupervisorModel,
    trainingPlanSupervisorEnabled:
      current.trainingPlanSupervisorEnabled === previousPersisted.trainingPlanSupervisorEnabled
        ? persistedDraft.trainingPlanSupervisorEnabled
        : current.trainingPlanSupervisorEnabled,
  };
}

export function useAiAgentsDraftSync(persistedDraft: DraftState) {
  const [draft, setDraft] = useState<DraftState>(persistedDraft);
  const [cleanDraft, setCleanDraft] = useState<DraftState>(persistedDraft);
  const previousPersistedRef = useRef(persistedDraft);

  useEffect(() => {
    const previousPersisted = previousPersistedRef.current;

    setDraft((current) => mergePersistedDraft(current, previousPersisted, persistedDraft));
    setCleanDraft((current) => mergePersistedDraft(current, previousPersisted, persistedDraft));
    previousPersistedRef.current = persistedDraft;
  }, [persistedDraft]);

  return {
    draft,
    setDraft,
    cleanDraft,
    setCleanDraft,
  };
}
