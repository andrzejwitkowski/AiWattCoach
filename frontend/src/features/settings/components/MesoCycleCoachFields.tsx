import type { AiAgentsDraftState } from '../aiAgentsDraft';
import type { ProviderOption } from '../llmProviders';
import { ModelField, ProviderSelectField, providerSuggestedModels } from './ProviderSelectField';

type MesoCycleCoachFieldsProps = {
  draft: AiAgentsDraftState;
  mesoProviderOption: ProviderOption | undefined;
  onProviderChange: (value: string) => void;
  onModelChange: (value: string) => void;
};

export function MesoCycleCoachFields({
  draft,
  mesoProviderOption,
  onProviderChange,
  onModelChange,
}: MesoCycleCoachFieldsProps) {
  return (
    <div className="mt-8 rounded-xl border border-white/10 bg-slate-950/40 p-4">
      <h3 className="text-sm font-semibold text-white">Meso Cycle Coach</h3>
      <p className="mt-1 text-xs text-slate-400">
        Optional override for 30-day meso plan generation. Leave empty to use the active provider and
        model above.
      </p>
      <div className="mt-4 grid gap-4 md:grid-cols-2">
        <ProviderSelectField
          id="meso-cycle-provider"
          label="Meso Provider"
          value={draft.mesoCycleProvider}
          emptyOptionLabel="Use active provider"
          onChange={onProviderChange}
        />
        <ModelField
          id="meso-cycle-model"
          label="Meso Model"
          value={draft.mesoCycleModel}
          placeholder={draft.mesoCycleProvider ? 'Model for meso generation' : 'Uses active model when empty'}
          suggestedModels={providerSuggestedModels(mesoProviderOption)}
          onChange={onModelChange}
        />
      </div>
    </div>
  );
}
