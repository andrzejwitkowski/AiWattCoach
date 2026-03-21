import type { AdminSystemInfo } from '../../auth/api/auth';

type ProtectedSystemInfoCardProps = {
  systemInfo: AdminSystemInfo | null;
};

export function ProtectedSystemInfoCard({ systemInfo }: ProtectedSystemInfoCardProps) {
  return (
    <section className="rounded-[2rem] border border-white/15 bg-white/8 p-8 backdrop-blur">
      <p className="text-sm font-semibold uppercase tracking-[0.3em] text-amber-200">Protected system info</p>
      <h2 className="mt-4 font-serif text-3xl text-white">Admin-only payload</h2>
      <p className="mt-4 max-w-2xl leading-7 text-slate-300">
        This data comes from the RBAC-protected backend route and replaces the old public start page
        as the operational admin entry point.
      </p>

      <dl className="mt-8 grid gap-4 rounded-3xl border border-white/10 bg-slate-950/60 p-5 text-sm text-slate-200">
        <div>
          <dt className="text-slate-400">Application</dt>
          <dd className="mt-1 font-medium text-white">{systemInfo?.appName ?? 'loading'}</dd>
        </div>
        <div>
          <dt className="text-slate-400">Mongo database</dt>
          <dd className="mt-1 font-medium text-white">{systemInfo?.mongoDatabase ?? 'loading'}</dd>
        </div>
      </dl>
    </section>
  );
}
