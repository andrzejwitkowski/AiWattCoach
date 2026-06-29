import type { ProviderOption } from '../llmProviders';
import { ModelField, ProviderSelectField, providerSuggestedModels } from './ProviderSelectField';

type OptionalProviderOverrideFieldsProps = {
  title: string;
  description: string;
  providerId: string;
  modelId: string;
  providerValue: string;
  modelValue: string;
  providerOption: ProviderOption | undefined;
  emptyOptionLabel?: string;
  modelPlaceholder?: string;
  onProviderChange: (value: string) => void;
  onModelChange: (value: string) => void;
};

export function OptionalProviderOverrideFields({
  title,
  description,
  providerId,
  modelId,
  providerValue,
  modelValue,
  providerOption,
  emptyOptionLabel = 'Use active provider',
  modelPlaceholder,
  onProviderChange,
  onModelChange,
}: OptionalProviderOverrideFieldsProps) {
  return (
    <div className="mt-8 rounded-xl border border-white/10 bg-slate-950/40 p-4">
      <h3 className="text-sm font-semibold text-white">{title}</h3>
      <p className="mt-1 text-xs text-slate-400">{description}</p>
      <div className="mt-4 grid gap-4 md:grid-cols-2">
        <ProviderSelectField
          id={providerId}
          label="Provider"
          value={providerValue}
          emptyOptionLabel={emptyOptionLabel}
          onChange={onProviderChange}
        />
        <ModelField
          id={modelId}
          label="Model"
          value={modelValue}
          placeholder={
            modelPlaceholder ??
            (providerValue ? 'Model for this override' : 'Uses active model when empty')
          }
          suggestedModels={providerSuggestedModels(providerOption)}
          onChange={onModelChange}
        />
      </div>
    </div>
  );
}
