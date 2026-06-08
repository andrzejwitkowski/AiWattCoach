import type { ProviderOption } from '../llmProviders';
import { ModelField, ProviderSelectField, providerSuggestedModels } from './ProviderSelectField';

type ProviderModelFieldsProps = {
  providerId: string;
  modelId: string;
  selectedProvider: string;
  selectedModel: string;
  selectedProviderOption: ProviderOption | undefined;
  onProviderChange: (value: string) => void;
  onModelChange: (value: string) => void;
};

export function ProviderModelFields({
  providerId,
  modelId,
  selectedProvider,
  selectedModel,
  selectedProviderOption,
  onProviderChange,
  onModelChange,
}: ProviderModelFieldsProps) {
  return (
    <div className="mt-6 grid gap-4 md:grid-cols-2">
      <ProviderSelectField
        id={providerId}
        label="Active Provider"
        value={selectedProvider}
        emptyOptionLabel="Choose provider"
        onChange={onProviderChange}
      />
      <ModelField
        id={modelId}
        label="Model"
        value={selectedModel}
        placeholder="gpt-4o-mini or openai/gpt-4o-mini"
        suggestedModels={providerSuggestedModels(selectedProviderOption)}
        onChange={onModelChange}
      />
    </div>
  );
}
