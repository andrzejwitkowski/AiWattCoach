import type { RefObject } from 'react';
import { AlertCircle, Bot, CheckCircle2, Eye, EyeOff, RefreshCw, Save } from 'lucide-react';

import { PROVIDER_OPTIONS, type AiAgentsCardStatus, type DraftState } from './AiAgentsCard.shared';

export type ApiKeyFieldConfig = {
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

type ProviderModelSectionProps = {
  draft: DraftState;
  suggestedModels: string[];
  validationMessage: string | null;
  onProviderChange: (value: string) => void;
  onModelChange: (value: string) => void;
};

type TrainingPlanSupervisorSectionProps = {
  enabled: boolean;
  selectedModel: string;
  supervisorModelOptions: string[];
  onEnabledChange: (enabled: boolean) => void;
  onModelChange: (value: string) => void;
};

type StatusBannerProps = {
  status: AiAgentsCardStatus | null;
};

type ActionButtonsProps = {
  isSaving: boolean;
  isTesting: boolean;
  canTest: boolean;
  canSave: boolean;
  hasDirtyDraft: boolean;
  onTest: () => void;
  onSave: () => void;
};

type SupervisorGeminiKeyModalProps = {
  open: boolean;
  dialogRef: RefObject<HTMLDivElement | null>;
  closeButtonRef: RefObject<HTMLButtonElement | null>;
  onClose: () => void;
};

export function AiAgentsCardHeader() {
  return (
    <>
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
    </>
  );
}

export function ProviderModelSection({
  draft,
  suggestedModels,
  validationMessage,
  onProviderChange,
  onModelChange,
}: ProviderModelSectionProps) {
  return (
    <>
      <div className="mt-6 grid gap-4 md:grid-cols-2">
        <div>
          <label htmlFor="ai-provider" className="mb-2 block text-xs uppercase tracking-widest text-slate-400">
            Active Provider
          </label>
          <select
            id="ai-provider"
            className="w-full rounded-xl border border-white/10 bg-slate-900/60 px-4 py-3 text-sm text-slate-200 focus:border-cyan-400/50 focus:outline-none"
            value={draft.selectedProvider}
            onChange={(event) => onProviderChange(event.target.value)}
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
            onChange={(event) => onModelChange(event.target.value)}
          />
          {suggestedModels.length > 0 ? (
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
                  onClick={() => onModelChange(model)}
                >
                  {model}
                </button>
              ))}
            </div>
          ) : null}
        </div>
      </div>

      {validationMessage ? (
        <div className="mt-4 rounded-xl border border-amber-400/20 bg-amber-400/10 px-4 py-3 text-sm text-amber-100">
          {validationMessage}
        </div>
      ) : null}
    </>
  );
}

export function TrainingPlanSupervisorSection({
  enabled,
  selectedModel,
  supervisorModelOptions,
  onEnabledChange,
  onModelChange,
}: TrainingPlanSupervisorSectionProps) {
  return (
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
          <span>{enabled ? 'Enabled' : 'Disabled'}</span>
          <input
            aria-label="Enable training plan supervisor"
            type="checkbox"
            className="h-4 w-4 rounded border-white/10 bg-slate-900/60 text-cyan-400 focus:ring-cyan-400/50"
            checked={enabled}
            onChange={(event) => onEnabledChange(event.target.checked)}
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
          value={selectedModel}
          onChange={(event) => onModelChange(event.target.value)}
        >
          {supervisorModelOptions.map((model) => (
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
  );
}

export function ApiKeysSection({ fields }: { fields: ApiKeyFieldConfig[] }) {
  return (
    <div className="mt-6 space-y-4">
      {fields.map((field) => (
        <ApiKeyField key={field.id} {...field} />
      ))}
    </div>
  );
}

export function StatusBanner({ status }: StatusBannerProps) {
  if (!status) {
    return null;
  }

  const statusClasses =
    status.tone === 'success'
      ? 'border-emerald-400/30 bg-emerald-500/10 text-emerald-200'
      : status.tone === 'error'
        ? 'border-red-500/30 bg-red-500/10 text-red-200'
        : 'border-cyan-400/20 bg-cyan-400/10 text-cyan-100';
  const StatusIcon =
    status.tone === 'success' ? CheckCircle2 : status.tone === 'error' ? AlertCircle : RefreshCw;

  return (
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
  );
}

export function ActionButtons({
  isSaving,
  isTesting,
  canTest,
  canSave,
  hasDirtyDraft,
  onTest,
  onSave,
}: ActionButtonsProps) {
  return (
    <div className="mt-6 flex gap-3">
      <button
        className="flex flex-1 items-center justify-center gap-2 rounded-xl border border-cyan-400/30 bg-transparent py-3 text-sm font-semibold text-cyan-300 transition hover:bg-cyan-400/10 disabled:cursor-not-allowed disabled:opacity-60"
        onClick={onTest}
        disabled={isSaving || isTesting || !canTest}
        type="button"
      >
        <RefreshCw size={15} className={isTesting ? 'animate-spin' : undefined} />
        {isTesting ? 'Testing...' : 'Test Connection'}
      </button>
      <button
        className="flex flex-1 items-center justify-center gap-2 rounded-xl bg-cyan-400 py-3 text-sm font-semibold text-slate-950 transition hover:bg-cyan-300 disabled:cursor-not-allowed disabled:opacity-60"
        onClick={onSave}
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
  );
}

export function SupervisorGeminiKeyModal({
  open,
  dialogRef,
  closeButtonRef,
  onClose,
}: SupervisorGeminiKeyModalProps) {
  if (!open) {
    return null;
  }

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-[#05070a]/78 px-4 py-6 backdrop-blur-sm"
      onClick={onClose}
    >
      <div
        ref={dialogRef}
        role="dialog"
        aria-modal="true"
        aria-labelledby="supervisor-gemini-key-title"
        tabIndex={-1}
        className="w-full max-w-md rounded-[1.75rem] border border-white/8 bg-[linear-gradient(180deg,rgba(28,32,36,0.98),rgba(15,18,20,0.98))] p-6 shadow-[0_40px_120px_rgba(0,0,0,0.58)]"
        onClick={(event) => {
          event.stopPropagation();
        }}
      >
        <h3 id="supervisor-gemini-key-title" className="text-lg font-bold text-white">
          Gemini API key required
        </h3>
        <p className="mt-3 text-sm leading-relaxed text-slate-300">
          Training plan supervisor uses Gemini Batch API. Add a Gemini API key before enabling the
          supervisor.
        </p>
        <div className="mt-5 flex justify-end">
          <button
            ref={closeButtonRef}
            type="button"
            className="rounded-xl bg-cyan-400 px-4 py-2 text-sm font-semibold text-slate-950 transition hover:bg-cyan-300"
            onClick={onClose}
          >
            OK
          </button>
        </div>
      </div>
    </div>
  );
}

type ApiKeyFieldProps = ApiKeyFieldConfig;

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
      {configured ? <p className="mt-1 text-xs text-emerald-400">API key is configured</p> : null}
    </div>
  );
}
