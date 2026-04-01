import { useEffect, useMemo, useRef, useState } from 'react';
import { AlertCircle, Bot, CheckCircle2, Eye, EyeOff, RefreshCw, Save } from 'lucide-react';
import type { TestAiAgentsConnectionResponse, UserSettingsResponse } from '../types';
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
  selectedProvider: string;
  selectedModel: string;
};

function clearDraftApiKeys(draft: DraftState): DraftState {
  return {
    ...draft,
    openaiApiKey: '',
    geminiApiKey: '',
    openrouterApiKey: '',
  };
}

export function AiAgentsCard({ settings, apiBaseUrl, onSave }: AiAgentsCardProps) {
  const aiAgents = settings.aiAgents;
  const persistedDraft = useMemo(
    () => ({
      openaiApiKey: aiAgents.openaiApiKey ?? '',
      geminiApiKey: aiAgents.geminiApiKey ?? '',
      openrouterApiKey: aiAgents.openrouterApiKey ?? '',
      selectedProvider: aiAgents.selectedProvider ?? '',
      selectedModel: aiAgents.selectedModel ?? '',
    }),
    [
      aiAgents.geminiApiKey,
      aiAgents.openaiApiKey,
      aiAgents.openrouterApiKey,
      aiAgents.selectedModel,
      aiAgents.selectedProvider,
    ],
  );
  const [draft, setDraft] = useState<DraftState>(persistedDraft);
  const [cleanDraft, setCleanDraft] = useState<DraftState>(persistedDraft);
  const [showOpenai, setShowOpenai] = useState(false);
  const [showGemini, setShowGemini] = useState(false);
  const [showOpenrouter, setShowOpenrouter] = useState(false);
  const [isSaving, setIsSaving] = useState(false);
  const [isTesting, setIsTesting] = useState(false);
  const [status, setStatus] = useState<{
    tone: 'neutral' | 'success' | 'error';
    label: string;
    message: string;
  } | null>(null);
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
      selectedProvider:
        current.selectedProvider === previousPersisted.selectedProvider
          ? persistedDraft.selectedProvider
          : current.selectedProvider,
      selectedModel:
        current.selectedModel === previousPersisted.selectedModel
          ? persistedDraft.selectedModel
          : current.selectedModel,
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
      selectedProvider:
        current.selectedProvider === previousPersisted.selectedProvider
          ? persistedDraft.selectedProvider
          : current.selectedProvider,
      selectedModel:
        current.selectedModel === previousPersisted.selectedModel
          ? persistedDraft.selectedModel
          : current.selectedModel,
    }));
    previousPersistedRef.current = persistedDraft;
  }, [persistedDraft]);

  const hasDirtyDraft =
    draft.openaiApiKey !== cleanDraft.openaiApiKey ||
    draft.geminiApiKey !== cleanDraft.geminiApiKey ||
    draft.openrouterApiKey !== cleanDraft.openrouterApiKey ||
    draft.selectedProvider !== cleanDraft.selectedProvider ||
    draft.selectedModel !== cleanDraft.selectedModel;
  const hasAnyPersistedConnectionValue =
    aiAgents.openaiApiKeySet ||
    aiAgents.geminiApiKeySet ||
    aiAgents.openrouterApiKeySet ||
    Boolean(aiAgents.selectedProvider) ||
    Boolean(aiAgents.selectedModel);
  const canSave =
    draft.selectedProvider.trim().length > 0 ||
    draft.selectedModel.trim().length > 0 ||
    draft.openaiApiKey.trim().length > 0 ||
    draft.geminiApiKey.trim().length > 0 ||
    draft.openrouterApiKey.trim().length > 0;
  const canTest = canSave || hasAnyPersistedConnectionValue;

  const visibleRequest = useMemo(() => {
    const request: Record<string, string> = {};
    const trimmedOpenai = draft.openaiApiKey.trim();
    const trimmedGemini = draft.geminiApiKey.trim();
    const trimmedOpenrouter = draft.openrouterApiKey.trim();
    const trimmedProvider = draft.selectedProvider.trim();
    const trimmedModel = draft.selectedModel.trim();

    if (trimmedOpenai && trimmedOpenai !== persistedDraft.openaiApiKey) {
      request.openaiApiKey = trimmedOpenai;
    }
    if (trimmedGemini && trimmedGemini !== persistedDraft.geminiApiKey) {
      request.geminiApiKey = trimmedGemini;
    }
    if (trimmedOpenrouter && trimmedOpenrouter !== persistedDraft.openrouterApiKey) {
      request.openrouterApiKey = trimmedOpenrouter;
    }
    if (trimmedProvider && trimmedProvider !== persistedDraft.selectedProvider) {
      request.selectedProvider = trimmedProvider;
    }
    if (trimmedModel && trimmedModel !== persistedDraft.selectedModel) {
      request.selectedModel = trimmedModel;
    }

    return request;
  }, [draft, persistedDraft]);

  const clearTestStatusIfNeeded = () => {
    testRunIdRef.current += 1;
    setIsTesting(false);
    setStatus(null);
  };

  const setStatusFromTest = (result: TestAiAgentsConnectionResponse) => {
    setStatus({
      tone: result.connected ? 'success' : 'error',
      label: result.connected ? 'OK' : 'FAILED',
      message: result.message,
    });
  };

  const updateDraft = (field: keyof DraftState, value: string) => {
    clearTestStatusIfNeeded();
    setDraft((current) => ({ ...current, [field]: value }));
  };

  const handleSave = async () => {
    if (!canSave) return;
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
        message: 'AI provider settings saved.',
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
      message: 'Testing current AI provider values...',
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
        Choose the provider, set the model, and manage provider-specific API keys. Use the test
        action to validate the current selection before saving.
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
            onChange={(event) => updateDraft('selectedProvider', event.target.value)}
          >
            <option value="">Choose provider</option>
            <option value="openai">OpenAI</option>
            <option value="gemini">Gemini</option>
            <option value="openrouter">OpenRouter</option>
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
          onVisibilityChange={() => setShowOpenrouter((value) => !value)}
          onChange={(value) => updateDraft('openrouterApiKey', value)}
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
  onVisibilityChange,
  onChange,
}: ApiKeyFieldProps) {
  return (
    <div>
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
      {configured && <p className="mt-1.5 text-xs text-emerald-400">API key is configured</p>}
    </div>
  );
}
