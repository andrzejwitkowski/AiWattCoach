import { useSettings } from '../features/settings/context/SettingsContext';
import { AiAgentsCard } from '../features/settings/components/AiAgentsCard';
import { AvailabilityCard } from '../features/settings/components/AvailabilityCard';
import { AthleteSummaryCard } from '../features/settings/components/AthleteSummaryCard';
import { CyclingSettingsCard } from '../features/settings/components/CyclingSettingsCard';
import { IntervalsCard } from '../features/settings/components/IntervalsCard';
import { OptionsCard } from '../features/settings/components/OptionsCard';

type SettingsPageProps = {
  apiBaseUrl: string;
};

export function SettingsPage({ apiBaseUrl }: SettingsPageProps) {
  const { settings, isLoading, error, refreshSettings, setSettings } = useSettings();

  if (isLoading) {
    return (
      <div className="flex items-center justify-center py-16">
        <div className="h-8 w-8 animate-spin rounded-full border-2 border-cyan-500 border-t-transparent" />
      </div>
    );
  }

  if (error && !settings) {
    return (
      <div className="rounded-2xl border border-red-500/30 bg-red-500/10 p-6 text-center">
        <p className="text-red-400">Failed to load settings: {error}</p>
        <button
          type="button"
          className="mt-3 rounded-lg bg-red-500/20 px-4 py-2 text-sm text-red-300 hover:bg-red-500/30"
          onClick={() => { void refreshSettings(); }}
        >
          Retry
        </button>
      </div>
    );
  }

  if (!settings) {
    return (
      <div className="rounded-2xl border border-white/10 bg-white/5 p-6 text-center">
        <p className="text-slate-400">No settings found.</p>
      </div>
    );
  }

  function handleSave() {
    void refreshSettings();
  }

  return (
    <section className="space-y-6">
      <div className="grid gap-6 lg:grid-cols-2">
        <AiAgentsCard
          settings={settings}
          apiBaseUrl={apiBaseUrl}
          onSave={handleSave}
        />
        <IntervalsCard
          settings={settings}
          apiBaseUrl={apiBaseUrl}
          onSave={handleSave}
        />
      </div>
      <AthleteSummaryCard settings={settings} apiBaseUrl={apiBaseUrl} />
      <AvailabilityCard
        settings={settings}
        apiBaseUrl={apiBaseUrl}
        onSave={(updatedSettings) => {
          if (updatedSettings) {
            setSettings(updatedSettings);
          }
          handleSave();
        }}
      />
      <OptionsCard
        settings={settings}
        apiBaseUrl={apiBaseUrl}
        onSave={handleSave}
      />
      <CyclingSettingsCard
        settings={settings}
        apiBaseUrl={apiBaseUrl}
        onSave={handleSave}
      />
    </section>
  );
}
