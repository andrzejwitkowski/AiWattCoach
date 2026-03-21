import type { BackendStatus } from '../../../lib/api/system';
import { getStatusPanelClass } from '../../../lib/statusUi';

type BackendStatusCardProps = {
  backendStatus: BackendStatus;
};

export function BackendStatusCard({ backendStatus }: BackendStatusCardProps) {
  const statusPanelClass = getStatusPanelClass(backendStatus.state);

  return (
    <aside
      className={`rounded-[2rem] border p-6 shadow-[0_20px_60px_rgba(15,23,42,0.2)] backdrop-blur ${statusPanelClass}`}
    >
      <p className="text-sm font-semibold uppercase tracking-[0.3em] text-cyan-200">Backend status</p>
      <p className="mt-4 text-3xl font-semibold text-white">{backendStatus.state}</p>
      <dl className="mt-6 grid gap-4 text-sm text-slate-200">
        <div>
          <dt className="text-slate-400">Service</dt>
          <dd className="mt-1 font-medium text-white">{backendStatus.health.service}</dd>
        </div>
        <div>
          <dt className="text-slate-400">Health</dt>
          <dd className="mt-1 font-medium text-white">{backendStatus.health.status}</dd>
        </div>
        <div>
          <dt className="text-slate-400">Readiness</dt>
          <dd className="mt-1 font-medium text-white">{backendStatus.readiness.status}</dd>
        </div>
        <div>
          <dt className="text-slate-400">Checked</dt>
          <dd className="mt-1 font-medium text-white">{backendStatus.checkedAtLabel}</dd>
        </div>
      </dl>
    </aside>
  );
}
