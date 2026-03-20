import type { BackendStatus } from '../lib/api/system';

type SettingsPageProps = {
  apiBaseUrlLabel: string;
  backendStatus: BackendStatus;
  onRefresh: () => void;
  isRefreshing: boolean;
};

export function SettingsPage({
  apiBaseUrlLabel,
  backendStatus,
  onRefresh,
  isRefreshing
}: SettingsPageProps) {
  const readinessPanelClass =
    backendStatus.state === 'online'
      ? 'border-emerald-300/20 bg-emerald-300/10 text-emerald-100'
      : backendStatus.state === 'degraded'
        ? 'border-amber-300/25 bg-amber-300/12 text-amber-100'
        : backendStatus.state === 'loading'
          ? 'border-slate-300/15 bg-slate-300/10 text-slate-100'
          : 'border-rose-300/25 bg-rose-300/12 text-rose-100';

  return (
    <section className="grid gap-6 xl:grid-cols-[minmax(0,1.3fr)_minmax(18rem,1fr)]">
      <div className="rounded-[2rem] border border-white/15 bg-white/8 p-8 backdrop-blur">
        <p className="text-sm font-semibold uppercase tracking-[0.3em] text-amber-200">Settings</p>
        <h2 className="mt-4 font-serif text-3xl text-white">Configuration entry point</h2>
        <p className="mt-4 max-w-2xl leading-7 text-slate-300">
          This screen anchors future athlete preferences and integration settings. For now it proves
          frontend-to-backend communication with a real API target.
        </p>

        <div className="mt-8 rounded-3xl border border-white/10 bg-slate-950/60 p-5">
          <p className="text-xs uppercase tracking-[0.3em] text-slate-400">API base URL</p>
          <p className="mt-3 break-all text-base font-medium text-cyan-200">{apiBaseUrlLabel}</p>
        </div>
      </div>

      <aside className={`rounded-[2rem] border p-6 backdrop-blur ${readinessPanelClass}`}>
        <p className="text-sm font-semibold uppercase tracking-[0.3em]">Readiness</p>
        <p className="mt-4 text-3xl font-semibold text-white">{backendStatus.readiness.status}</p>
        <p className="mt-3 text-sm text-slate-200">
          {backendStatus.readiness.reason ?? 'Backend reports ready for requests.'}
        </p>
        <button
          className="mt-6 rounded-full bg-white px-5 py-3 text-sm font-semibold text-slate-900 transition hover:bg-cyan-100 disabled:cursor-not-allowed disabled:opacity-60"
          disabled={isRefreshing}
          onClick={onRefresh}
          type="button"
        >
          {isRefreshing ? 'Refreshing...' : 'Re-check backend'}
        </button>
      </aside>
    </section>
  );
}
