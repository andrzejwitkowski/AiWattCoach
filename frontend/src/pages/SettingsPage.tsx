import { AiAgentsCard } from '../features/settings/components/AiAgentsCard';
import { CyclingSettingsCard } from '../features/settings/components/CyclingSettingsCard';
import { IntervalsCard } from '../features/settings/components/IntervalsCard';
import { OptionsCard } from '../features/settings/components/OptionsCard';
import { useSettings } from '../features/settings/context/SettingsProvider';

export function SettingsPage() {
  const { settings, isSaving, updateAiAgents, updateIntervals, updateOptions, updateCycling } =
    useSettings();

  return (
    <section className="space-y-6">
      <div className="grid gap-6 lg:grid-cols-2">
        <AiAgentsCard
          settings={settings.aiAgents}
          onSave={updateAiAgents}
          isSaving={!!isSaving.aiAgents}
        />
        <IntervalsCard
          settings={settings.intervals}
          onSave={updateIntervals}
          isSaving={!!isSaving.intervals}
        />
      </div>
      <OptionsCard settings={settings.options} onToggle={updateOptions} />
      <CyclingSettingsCard
        settings={settings.cycling}
        onSave={updateCycling}
        isSaving={!!isSaving.cycling}
      />
    </section>
  );
}
