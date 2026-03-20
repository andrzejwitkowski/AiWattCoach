import type { BackendStatus } from '../lib/api/system';

type HomePageProps = {
  backendStatus: BackendStatus;
};

export function HomePage({ backendStatus }: HomePageProps) {
  const statusPanelClass =
    backendStatus.state === 'online'
      ? 'border-cyan-300/20 bg-cyan-300/10'
      : backendStatus.state === 'degraded'
        ? 'border-amber-300/25 bg-amber-300/12'
        : backendStatus.state === 'loading'
          ? 'border-slate-300/15 bg-slate-300/10'
          : 'border-rose-300/25 bg-rose-300/12';

  return (
    <section className="grid gap-5 lg:grid-cols-[minmax(0,2fr)_minmax(18rem,1fr)]">
      <div className="rounded-[2rem] border border-white/15 bg-slate-950/70 p-8 shadow-[0_30px_80px_rgba(15,23,42,0.45)] backdrop-blur">
        <p className="text-sm font-semibold uppercase tracking-[0.35em] text-cyan-300">Ride smarter</p>
        <h2 className="mt-4 max-w-xl font-serif text-4xl leading-tight text-white md:text-5xl">
          Training decisions, backend reliability, and UI clarity in one place.
        </h2>
        <p className="mt-5 max-w-2xl text-base leading-7 text-slate-300 md:text-lg">
          The shell is live and connected to the Rust backend. Use it as the starting point for
          athlete settings, sync controls, and future planning workflows.
        </p>
      </div>

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
    </section>
  );
}
