import { useEffect, useMemo, useRef, useState } from 'react';
import { AlertCircle, Bot, CheckCircle2, Eye, EyeOff, RefreshCw, Save } from 'lucide-react';
import type { LlmProvider, TestAiAgentsConnectionResponse, UserSettingsResponse } from '../types';
import { testAiAgentsConnection, updateAiAgents } from '../api/settings';

type AiAgentsCardProps = {
  settings: UserSettingsResponse;
  apiBaseUrl: string;
  onSave: () => void;
};

type DraftState = {
  openaiApiKey: string;
  geminiApiKey: string;
  openrouterApiKey: string;
  deepseekApiKey: string;
  selectedProvider: string;
  selectedModel: string;
  trainingPlanSupervisorModel: string;
  trainingPlanSupervisorEnabled: boolean;
};

type ProviderOption = {
  value: LlmProvider;
  label: string;
  suggestedModels: string[];
};

const PROVIDER_OPTIONS: ProviderOption[] = [
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
];

const SUPERVISOR_MODELS = ['gemini-2.5-pro', 'gemini-2.5-flash', 'gemini-3-flash-preview'];

function clearDraftApiKeys(draft: DraftState): DraftState {
  return {
    ...draft,
    openaiApiKey: '',
    geminiApiKey: '',
    openrouterApiKey: '',
    deepseekApiKey: '',
  };
}

function getProviderOption(provider: string) {
  return PROVIDER_OPTIONS.find((option) => option.value === provider);
}

function getProviderKeyState(provider: string, draft: DraftState, aiAgents: UserSettingsResponse['aiAgents']) {
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
    default:
      return {
        draftValue: '',
        hasPersistedKey: false,
        label: 'Provider',
      };
  }
}

function buildTestStatusMessage(result: TestAiAgentsConnectionResponse) {
  const reusedSavedValues = [
    result.usedSavedApiKey ? 'saved key' : null,
    result.usedSavedProvider ? 'saved provider' : null,
    result.usedSavedModel ? 'saved model' : null,
  ].filter(Boolean);

  if (reusedSavedValues.length === 0) {
    return `${result.message} Tested the visible draft only.`;
  }

  return `${result.message} Used ${reusedSavedValues.join(', ')} for unchanged fields.`;
}

export function AiAgentsCard({ settings, apiBaseUrl, onSave }: AiAgentsCardProps) {
  const aiAgents = settings.aiAgents;
  const persistedDraft = useMemo(
    () => ({
      openaiApiKey: '',
      geminiApiKey: '',
      openrouterApiKey: '',
      deepseekApiKey: '',
      selectedProvider: aiAgents.selectedProvider ?? '',
      selectedModel: aiAgents.selectedModel ?? '',
      trainingPlanSupervisorModel: aiAgents.trainingPlanSupervisorModel ?? 'gemini-2.5-pro',
      trainingPlanSupervisorEnabled: aiAgents.trainingPlanSupervisorEnabled,
    }),
    [
      aiAgents.selectedModel,
      aiAgents.selectedProvider,
      aiAgents.trainingPlanSupervisorEnabled,
      aiAgents.trainingPlanSupervisorModel,
    ],
  );
  const [draft, setDraft] = useState<DraftState>(persistedDraft);
  const [cleanDraft, setCleanDraft] = useState<DraftState>(persistedDraft);
  const [showOpenai, setShowOpenai] = useState(false);
  const [showGemini, setShowGemini] = useState(false);
  const [showOpenrouter, setShowOpenrouter] = useState(false);
  const [showDeepseek, setShowDeepseek] = useState(false);
  const [isSaving, setIsSaving] = useState(false);
  const [isTesting, setIsTesting] = useState(false);
  const [status, setStatus] = useState<{
    tone: 'neutral' | 'success' | 'error';
    label: string;
    message: string;
  } | null>(null);
  const [showSupervisorGeminiKeyModal, setShowSupervisorGeminiKeyModal] = useState(false);
  const previousPersistedRef = useRef(persistedDraft);
  const testRunIdRef = useRef(0);

  useEffect(() => {
    const previousPersisted = previousPersistedRef.current;

    setDraft((current) => ({
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
    }));
    setCleanDraft((current) => ({
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
    }));
    previousPersistedRef.current = persistedDraft;
  }, [persistedDraft]);

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

  const visibleRequest = useMemo(() => {
    const request: Partial<Record<keyof DraftState, string | boolean | null>> = {};
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
    if (
      draft.trainingPlanSupervisorEnabled !== persistedDraft.trainingPlanSupervisorEnabled
    ) {
      request.trainingPlanSupervisorEnabled = draft.trainingPlanSupervisorEnabled;
    }
    if (trimmedSupervisorModel !== persistedDraft.trainingPlanSupervisorModel) {
      request.trainingPlanSupervisorModel = trimmedSupervisorModel.length > 0
        ? trimmedSupervisorModel
        : '';
    }

    return request;
  }, [draft, persistedDraft]);

  const selectedProviderOption = getProviderOption(draft.selectedProvider);
  const suggestedModels = selectedProviderOption?.suggestedModels ?? [];
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
    clearTestStatusIfNeeded();
    setDraft((current) => ({ ...current, [field]: value }));
  };

  const updateSupervisorEnabled = (enabled: boolean) => {
    clearTestStatusIfNeeded();
    setDraft((current) => ({
      ...current,
      trainingPlanSupervisorEnabled: enabled,
      trainingPlanSupervisorModel: current.trainingPlanSupervisorModel || 'gemini-2.5-pro',
    }));
  };

  const updateProvider = (value: string) => {
    clearTestStatusIfNeeded();
    setDraft((current) => {
      const previousOption = getProviderOption(current.selectedProvider);
      const nextOption = getProviderOption(value);
      const currentModel = current.selectedModel.trim();
      const shouldAutofillModel =
        Boolean(nextOption) && (!currentModel || previousOption?.suggestedModels.includes(currentModel));

      return {
        ...current,
        selectedProvider: value,
        selectedModel: shouldAutofillModel ? nextOption?.suggestedModels[0] ?? current.selectedModel : current.selectedModel,
      };
    });
  };

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

  const statusClasses =
    status?.tone === 'success'
      ? 'border-emerald-400/30 bg-emerald-500/10 text-emerald-200'
      : status?.tone === 'error'
        ? 'border-red-500/30 bg-red-500/10 text-red-200'
        : 'border-cyan-400/20 bg-cyan-400/10 text-cyan-100';
  const StatusIcon =
    status?.tone === 'success' ? CheckCircle2 : status?.tone === 'error' ? AlertCircle : RefreshCw;

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
        Choose the active provider, start from a recommended model, and keep only the matching API
        key in focus while you test the visible draft.
      </p>

      <p className="mt-2 text-xs text-slate-500">
        Suggested models are current examples. You can still type any supported model name.
      </p>

      <div className="mt-6 grid gap-4 md:grid-cols-2">
        <div>
          <label htmlFor="ai-provider" className="mb-2 block text-xs uppercase tracking-widest text-slate-400">
            Active Provider
          </label>
          <select
            id="ai-provider"
            className="w-full rounded-xl border border-white/10 bg-slate-900/60 px-4 py-3 text-sm text-slate-200 focus:border-cyan-400/50 focus:outline-none"
            value={draft.selectedProvider}
            onChange={(event) => updateProvider(event.target.value)}
          >
            <option value="">Choose provider</option>
            {PROVIDER_OPTIONS.map((option) => (
              <option key={option.value} value={option.value}>
                {option.label}
              </option>
            ))}
          </select>
        </div>

        <div>
          <label htmlFor="ai-model" className="mb-2 block text-xs uppercase tracking-widest text-slate-400">
            Model
          </label>
          <input
            id="ai-model"
            className="w-full rounded-xl border border-white/10 bg-slate-900/60 px-4 py-3 text-sm text-slate-200 placeholder:text-slate-600 focus:border-cyan-400/50 focus:outline-none"
            type="text"
            placeholder="gpt-4o-mini or openai/gpt-4o-mini"
            value={draft.selectedModel}
            onChange={(event) => updateDraft('selectedModel', event.target.value)}
          />
          {suggestedModels.length > 0 && (
            <div className="mt-2 flex flex-wrap gap-2">
              {suggestedModels.map((model) => (
                <button
                  key={model}
                  type="button"
                  className={`rounded-full border px-3 py-1 text-xs transition ${
                    draft.selectedModel.trim() === model
                      ? 'border-cyan-400/60 bg-cyan-400/15 text-cyan-200'
                      : 'border-white/10 bg-slate-900/60 text-slate-300 hover:border-cyan-400/30 hover:text-cyan-200'
                  }`}
                  onClick={() => updateDraft('selectedModel', model)}
                >
                  {model}
                </button>
              ))}
            </div>
          )}
        </div>
      </div>

      {validationMessage && (
        <div className="mt-4 rounded-xl border border-amber-400/20 bg-amber-400/10 px-4 py-3 text-sm text-amber-100">
          {validationMessage}
        </div>
      )}

      <div className="mt-6 rounded-2xl border border-white/10 bg-slate-950/30 p-4">
        <div className="flex items-start justify-between gap-4">
          <div>
            <p className="text-xs uppercase tracking-widest text-slate-400">Training Plan Supervisor</p>
            <p className="mt-2 text-sm text-slate-300">
              Run an async Gemini review pass after the worker-generated 14-day plan and allow
              supervised replacement when the reviewed plan is stronger.
            </p>
          </div>
          <label className="inline-flex items-center gap-3 text-sm font-medium text-slate-200">
            <span>{draft.trainingPlanSupervisorEnabled ? 'Enabled' : 'Disabled'}</span>
            <input
              aria-label="Enable training plan supervisor"
              type="checkbox"
              className="h-4 w-4 rounded border-white/10 bg-slate-900/60 text-cyan-400 focus:ring-cyan-400/50"
              checked={draft.trainingPlanSupervisorEnabled}
              onChange={(event) => updateSupervisorEnabled(event.target.checked)}
            />
          </label>
        </div>

        <div className="mt-4">
          <label
            htmlFor="training-plan-supervisor-model"
            className="mb-2 block text-xs uppercase tracking-widest text-slate-400"
          >
            Supervisor Model
          </label>
          <select
            id="training-plan-supervisor-model"
            aria-label="Training plan supervisor model"
            className="w-full rounded-xl border border-white/10 bg-slate-900/60 px-4 py-3 text-sm text-slate-200 focus:border-cyan-400/50 focus:outline-none"
            value={draft.trainingPlanSupervisorModel}
            onChange={(event) => updateDraft('trainingPlanSupervisorModel', event.target.value)}
          >
            {SUPERVISOR_MODELS.map((model) => (
              <option key={model} value={model}>
                {model}
              </option>
            ))}
          </select>
          <p className="mt-2 text-xs text-slate-500">
            Uses your Gemini API key. Default: <code>gemini-2.5-pro</code>.
          </p>
        </div>
      </div>

      <div className="mt-6 space-y-4">
        <ApiKeyField
          id="openai-api-key"
          label="OpenAI API Key"
          placeholder={aiAgents.openaiApiKeySet ? 'Already configured' : 'sk-...'}
          value={draft.openaiApiKey}
          visible={showOpenai}
          configured={aiAgents.openaiApiKeySet}
          emphasized={!draft.selectedProvider || draft.selectedProvider === 'openai'}
          helperText={
            draft.selectedProvider === 'openai'
              ? 'Used by the active provider.'
              : openaiHasKey
                ? 'Saved for quick provider switching.'
                : 'Optional unless you switch to this provider.'
          }
          onVisibilityChange={() => setShowOpenai((value) => !value)}
          onChange={(value) => updateDraft('openaiApiKey', value)}
        />
        <ApiKeyField
          id="gemini-api-key"
          label="Gemini API Key"
          placeholder={aiAgents.geminiApiKeySet ? 'Already configured' : 'AIza...'}
          value={draft.geminiApiKey}
          visible={showGemini}
          configured={aiAgents.geminiApiKeySet}
          emphasized={!draft.selectedProvider || draft.selectedProvider === 'gemini'}
          helperText={
            draft.selectedProvider === 'gemini'
              ? 'Used by the active provider.'
              : geminiHasKey
                ? 'Saved for quick provider switching.'
                : 'Optional unless you switch to this provider.'
          }
          onVisibilityChange={() => setShowGemini((value) => !value)}
          onChange={(value) => updateDraft('geminiApiKey', value)}
        />
        <ApiKeyField
          id="openrouter-api-key"
          label="OpenRouter API Key"
          placeholder={aiAgents.openrouterApiKeySet ? 'Already configured' : 'sk-or-...'}
          value={draft.openrouterApiKey}
          visible={showOpenrouter}
          configured={aiAgents.openrouterApiKeySet}
          emphasized={!draft.selectedProvider || draft.selectedProvider === 'openrouter'}
          helperText={
            draft.selectedProvider === 'openrouter'
              ? 'Used by the active provider.'
              : openrouterHasKey
                ? 'Saved for quick provider switching.'
                : 'Optional unless you switch to this provider.'
          }
          onVisibilityChange={() => setShowOpenrouter((value) => !value)}
          onChange={(value) => updateDraft('openrouterApiKey', value)}
        />
        <ApiKeyField
          id="deepseek-api-key"
          label="DeepSeek API Key"
          placeholder={aiAgents.deepseekApiKeySet ? 'Already configured' : 'sk-...'}
          value={draft.deepseekApiKey}
          visible={showDeepseek}
          configured={aiAgents.deepseekApiKeySet}
          emphasized={!draft.selectedProvider || draft.selectedProvider === 'deepseek'}
          helperText={
            draft.selectedProvider === 'deepseek'
              ? 'Used by the active provider.'
              : deepseekHasKey
                ? 'Saved for quick provider switching.'
                : 'Optional unless you switch to this provider.'
          }
          onVisibilityChange={() => setShowDeepseek((value) => !value)}
          onChange={(value) => updateDraft('deepseekApiKey', value)}
        />
      </div>

      {status && (
        <div className={`mt-4 rounded-xl border px-4 py-3 text-sm ${statusClasses}`}>
          <div className="flex items-start gap-3">
            <StatusIcon
              size={16}
              className={status.tone === 'neutral' ? 'mt-0.5 shrink-0 animate-spin' : 'mt-0.5 shrink-0'}
            />
            <div>
              <p className="text-[11px] font-semibold uppercase tracking-wider">{status.label}</p>
              <p className="mt-1">{status.message}</p>
            </div>
          </div>
        </div>
      )}

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

      {showSupervisorGeminiKeyModal ? (
        <div
          className="fixed inset-0 z-50 flex items-center justify-center bg-[#05070a]/78 px-4 py-6 backdrop-blur-sm"
          onClick={() => setShowSupervisorGeminiKeyModal(false)}
        >
          <div
            role="dialog"
            aria-modal="true"
            aria-labelledby="supervisor-gemini-key-title"
            className="w-full max-w-md rounded-[1.75rem] border border-white/8 bg-[linear-gradient(180deg,rgba(28,32,36,0.98),rgba(15,18,20,0.98))] p-6 shadow-[0_40px_120px_rgba(0,0,0,0.58)]"
            onClick={(event) => {
              event.stopPropagation();
            }}
          >
            <h3 id="supervisor-gemini-key-title" className="text-lg font-bold text-white">
              Gemini API key required
            </h3>
            <p className="mt-3 text-sm leading-relaxed text-slate-300">
              Training plan supervisor uses Gemini Batch API. Add a Gemini API key before enabling
              the supervisor.
            </p>
            <div className="mt-5 flex justify-end">
              <button
                type="button"
                className="rounded-xl bg-cyan-400 px-4 py-2 text-sm font-semibold text-slate-950 transition hover:bg-cyan-300"
                onClick={() => setShowSupervisorGeminiKeyModal(false)}
              >
                OK
              </button>
            </div>
          </div>
        </div>
      ) : null}
    </div>
  );
}

type ApiKeyFieldProps = {
  id: string;
  label: string;
  placeholder: string;
  value: string;
  visible: boolean;
  configured: boolean;
  emphasized: boolean;
  helperText: string;
  onVisibilityChange: () => void;
  onChange: (value: string) => void;
};

function ApiKeyField({
  id,
  label,
  placeholder,
  value,
  visible,
  configured,
  emphasized,
  helperText,
  onVisibilityChange,
  onChange,
}: ApiKeyFieldProps) {
  return (
    <div className={emphasized ? 'opacity-100' : 'opacity-60'}>
      <label htmlFor={id} className="mb-2 block text-xs uppercase tracking-widest text-slate-400">
        {label}
      </label>
      <div className="relative">
        <input
          id={id}
          className="w-full rounded-xl border border-white/10 bg-slate-900/60 px-4 py-3 pr-10 text-sm text-slate-200 placeholder:text-slate-600 focus:border-cyan-400/50 focus:outline-none"
          type={visible ? 'text' : 'password'}
          placeholder={placeholder}
          value={value}
          onChange={(event) => onChange(event.target.value)}
        />
        <button
          className="absolute right-3 top-1/2 -translate-y-1/2 text-slate-400 transition hover:text-slate-200"
          onClick={onVisibilityChange}
          type="button"
          aria-label={visible ? 'Hide key' : 'Show key'}
        >
          {visible ? <EyeOff size={16} /> : <Eye size={16} />}
        </button>
      </div>
      <p className="mt-1.5 text-xs text-slate-400">{helperText}</p>
      {configured && <p className="mt-1 text-xs text-emerald-400">API key is configured</p>}
    </div>
  );
}
