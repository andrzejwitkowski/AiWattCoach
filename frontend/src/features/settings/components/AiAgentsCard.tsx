import { useRef } from 'react';
import { useDialogFocusTrap } from '../../../lib/useDialogFocusTrap';
import type { UserSettingsResponse } from '../types';
import {
  ActionButtons,
  AiAgentsCardHeader,
  ApiKeysSection,
  ProviderModelSection,
  StatusBanner,
  SupervisorGeminiKeyModal,
  TrainingPlanSupervisorSection,
} from './AiAgentsCardSections';
import { useAiAgentsCard } from './useAiAgentsCard';

type AiAgentsCardProps = {
  settings: UserSettingsResponse;
  apiBaseUrl: string;
  onSave: () => void;
};

export function AiAgentsCard({ settings, apiBaseUrl, onSave }: AiAgentsCardProps) {
  const {
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
  } = useAiAgentsCard({ settings, apiBaseUrl, onSave });
  const supervisorDialogRef = useRef<HTMLDivElement>(null);
  const supervisorDialogCloseButtonRef = useRef<HTMLButtonElement>(null);

  useDialogFocusTrap(
    showSupervisorGeminiKeyModal,
    supervisorDialogRef,
    supervisorDialogCloseButtonRef,
  );

  return (
    <div className="rounded-2xl border border-white/10 bg-white/5 p-6 backdrop-blur">
      <AiAgentsCardHeader />
      <ProviderModelSection
        draft={draft}
        suggestedModels={suggestedModels}
        validationMessage={validationMessage}
        onProviderChange={updateProvider}
        onModelChange={(value) => updateDraft('selectedModel', value)}
      />
      <TrainingPlanSupervisorSection
        enabled={draft.trainingPlanSupervisorEnabled}
        selectedModel={draft.trainingPlanSupervisorModel}
        supervisorModelOptions={supervisorModelOptions}
        onEnabledChange={updateSupervisorEnabled}
        onModelChange={(value) => updateDraft('trainingPlanSupervisorModel', value)}
      />
      <ApiKeysSection fields={apiKeyFields} />
      <StatusBanner status={status} />
      <ActionButtons
        isSaving={isSaving}
        isTesting={isTesting}
        canTest={canTest}
        canSave={canSave}
        hasDirtyDraft={hasDirtyDraft}
        onTest={() => {
          void handleTest();
        }}
        onSave={() => {
          void handleSave();
        }}
      />
      <SupervisorGeminiKeyModal
        open={showSupervisorGeminiKeyModal}
        dialogRef={supervisorDialogRef}
        closeButtonRef={supervisorDialogCloseButtonRef}
        onClose={() => setShowSupervisorGeminiKeyModal(false)}
      />
    </div>
  );
}
