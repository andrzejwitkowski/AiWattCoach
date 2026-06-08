import { Bot, RefreshCw, Save } from 'lucide-react';

import { useAiAgentsCard } from '../hooks/useAiAgentsCard';
import type { UserSettingsResponse } from '../types';
import { AiAgentsApiKeyFields } from './AiAgentsApiKeyFields';
import { MesoCycleCoachFields } from './MesoCycleCoachFields';
import { ProviderModelFields } from './ProviderModelFields';
import { SettingsStatusBanner } from './SettingsStatusBanner';

type AiAgentsCardProps = {
  settings: UserSettingsResponse;
  apiBaseUrl: string;
  onSave: () => void;
};

export function AiAgentsCard({ settings, apiBaseUrl, onSave }: AiAgentsCardProps) {
  const {
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
  } = useAiAgentsCard({ settings, apiBaseUrl, onSave });

  return (
    <div className="rounded-2xl border border-white/10 bg-white/5 p-6 backdrop-blur">
      <div className="flex items-start gap-4">
        <div className="flex h-10 w-10 shrink-0 items-center justify-center rounded-xl bg-slate-800">
          <Bot size={20} className="text-cyan-400" />
        </div>
        <div className="flex-1">
          <div className="flex flex-wrap items-center gap-2">
            <h2 className="text-xl font-bold text-white">AI Agents</h2>
            <span className="rounded-full bg-cyan-400/20 px-2 py-0.5 text-[10px] font-semibold uppercase tracking-wider text-cyan-300">
              BYOK
            </span>
          </div>
          <p className="mt-0.5 text-[10px] uppercase tracking-[0.2em] text-slate-500">
            Performance Intelligence
          </p>
        </div>
      </div>

      <p className="mt-4 text-sm leading-relaxed text-slate-300">
        Choose the active provider, start from a recommended model, and keep only the matching API key in
        focus while you test the visible draft.
      </p>

      <p className="mt-2 text-xs text-slate-500">
        Suggested models are current examples. You can still type any supported model name.
      </p>

      <ProviderModelFields
        providerId="ai-provider"
        modelId="ai-model"
        selectedProvider={draft.selectedProvider}
        selectedModel={draft.selectedModel}
        selectedProviderOption={selectedProviderOption}
        onProviderChange={updateProvider}
        onModelChange={(value) => updateDraft('selectedModel', value)}
      />

      <MesoCycleCoachFields
        draft={draft}
        mesoProviderOption={mesoProviderOption}
        onProviderChange={updateMesoProvider}
        onModelChange={(value) => updateDraft('mesoCycleModel', value)}
      />

      {validationMessage ? (
        <div className="mt-4 rounded-xl border border-amber-400/20 bg-amber-400/10 px-4 py-3 text-sm text-amber-100">
          {validationMessage}
        </div>
      ) : null}

      <AiAgentsApiKeyFields aiAgents={settings.aiAgents} draft={draft} onUpdate={updateDraft} />

      {status ? <SettingsStatusBanner status={status} /> : null}

      <div className="mt-6 flex gap-3">
        <button
          className="flex flex-1 items-center justify-center gap-2 rounded-xl border border-cyan-400/30 bg-transparent py-3 text-sm font-semibold text-cyan-300 transition hover:bg-cyan-400/10 disabled:cursor-not-allowed disabled:opacity-60"
          onClick={() => {
            void handleTest();
          }}
          disabled={isSaving || isTesting || !canTest}
          type="button"
        >
          <RefreshCw size={15} className={isTesting ? 'animate-spin' : undefined} />
          {isTesting ? 'Testing...' : 'Test Connection'}
        </button>
        <button
          className="flex flex-1 items-center justify-center gap-2 rounded-xl bg-cyan-400 py-3 text-sm font-semibold text-slate-950 transition hover:bg-cyan-300 disabled:cursor-not-allowed disabled:opacity-60"
          onClick={() => {
            void handleSave();
          }}
          disabled={isSaving || isTesting || !canSave || !hasDirtyDraft}
          type="button"
        >
          {isSaving ? (
            <>
              <RefreshCw size={15} className="animate-spin" />
              Saving...
            </>
          ) : (
            <>
              <Save size={15} />
              Save AI Config
            </>
          )}
        </button>
      </div>
    </div>
  );
}
