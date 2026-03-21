import type { BackendStatus } from '../../../lib/api/system';
import { getReadinessMessage, getStatusToneClass } from '../../../lib/statusUi';

type ApiConfigCardProps = {
  apiBaseUrlLabel: string;
  backendStatus: BackendStatus;
  onRefresh: () => void;
  isRefreshing: boolean;
};

export function ApiConfigCard({
  apiBaseUrlLabel,
  backendStatus,
  onRefresh,
  isRefreshing
}: ApiConfigCardProps) {
  const readinessPanelClass = getStatusToneClass(backendStatus.state);
  const readinessMessage = getReadinessMessage(backendStatus.state, backendStatus.readiness.reason);

  return (
    <section className="rounded-[2rem] border border-white/15 bg-white/8 p-8 backdrop-blur">
      <p className="text-sm font-semibold uppercase tracking-[0.3em] text-amber-200">API config</p>
      <h2 className="mt-4 font-serif text-3xl text-white">Admin diagnostics</h2>
      <p className="mt-4 max-w-2xl leading-7 text-slate-300">
        This area keeps the existing backend visibility and configuration probes available to
        administrators.
      </p>

      <div className="mt-8 rounded-3xl border border-white/10 bg-slate-950/60 p-5">
        <p className="text-xs uppercase tracking-[0.3em] text-slate-400">API base URL</p>
        <p className="mt-3 break-all text-base font-medium text-cyan-200">{apiBaseUrlLabel}</p>
      </div>

      <div className={`mt-6 rounded-[1.7rem] border p-6 backdrop-blur ${readinessPanelClass}`}>
        <p className="text-sm font-semibold uppercase tracking-[0.3em]">Readiness</p>
        <p className="mt-4 text-3xl font-semibold text-white">{backendStatus.readiness.status}</p>
        <p className="mt-3 text-sm text-slate-200">{readinessMessage}</p>
        <button
          className="mt-6 rounded-full bg-white px-5 py-3 text-sm font-semibold text-slate-900 transition hover:bg-cyan-100 disabled:cursor-not-allowed disabled:opacity-60"
          disabled={isRefreshing}
          onClick={onRefresh}
          type="button"
        >
          {isRefreshing ? 'Refreshing...' : 'Re-check backend'}
        </button>
      </div>
    </section>
  );
}
