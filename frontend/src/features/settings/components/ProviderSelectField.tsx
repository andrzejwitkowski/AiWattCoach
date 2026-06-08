import type { ProviderOption } from '../llmProviders';
import { PROVIDER_OPTIONS } from '../llmProviders';

type ProviderSelectFieldProps = {
  id: string;
  label: string;
  value: string;
  emptyOptionLabel: string;
  onChange: (value: string) => void;
};

export function ProviderSelectField({
  id,
  label,
  value,
  emptyOptionLabel,
  onChange,
}: ProviderSelectFieldProps) {
  return (
    <div>
      <label htmlFor={id} className="mb-2 block text-xs uppercase tracking-widest text-slate-400">
        {label}
      </label>
      <select
        id={id}
        className="w-full rounded-xl border border-white/10 bg-slate-900/60 px-4 py-3 text-sm text-slate-200 focus:border-cyan-400/50 focus:outline-none"
        value={value}
        onChange={(event) => onChange(event.target.value)}
      >
        <option value="">{emptyOptionLabel}</option>
        {PROVIDER_OPTIONS.map((option) => (
          <option key={option.value} value={option.value}>
            {option.label}
          </option>
        ))}
      </select>
    </div>
  );
}

type ModelFieldProps = {
  id: string;
  label: string;
  value: string;
  placeholder: string;
  suggestedModels: string[];
  onChange: (value: string) => void;
};

export function ModelField({
  id,
  label,
  value,
  placeholder,
  suggestedModels,
  onChange,
}: ModelFieldProps) {
  return (
    <div>
      <label htmlFor={id} className="mb-2 block text-xs uppercase tracking-widest text-slate-400">
        {label}
      </label>
      <input
        id={id}
        className="w-full rounded-xl border border-white/10 bg-slate-900/60 px-4 py-3 text-sm text-slate-200 placeholder:text-slate-600 focus:border-cyan-400/50 focus:outline-none"
        type="text"
        placeholder={placeholder}
        value={value}
        onChange={(event) => onChange(event.target.value)}
      />
      {suggestedModels.length > 0 ? (
        <div className="mt-2 flex flex-wrap gap-2">
          {suggestedModels.map((model) => (
            <button
              key={model}
              type="button"
              className={`rounded-full border px-3 py-1 text-xs transition ${
                value.trim() === model
                  ? 'border-cyan-400/60 bg-cyan-400/15 text-cyan-200'
                  : 'border-white/10 bg-slate-900/60 text-slate-300 hover:border-cyan-400/30 hover:text-cyan-200'
              }`}
              onClick={() => onChange(model)}
            >
              {model}
            </button>
          ))}
        </div>
      ) : null}
    </div>
  );
}

export function providerSuggestedModels(option: ProviderOption | undefined) {
  return option?.suggestedModels ?? [];
}
