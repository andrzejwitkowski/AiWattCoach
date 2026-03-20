import { NavLink, Outlet } from 'react-router-dom';

import type { BackendStatus } from '../lib/api/system';

type AppShellProps = {
  backendStatus: BackendStatus;
};

const navigationItems = [
  { to: '/', label: 'Overview' },
  { to: '/settings', label: 'Settings' }
];

export function AppShell({ backendStatus }: AppShellProps) {
  const statusAccentClass =
    backendStatus.state === 'online'
      ? 'border-cyan-300/20 bg-cyan-300/10 text-cyan-200'
      : backendStatus.state === 'degraded'
        ? 'border-amber-300/25 bg-amber-300/12 text-amber-100'
        : backendStatus.state === 'loading'
          ? 'border-slate-300/15 bg-slate-300/10 text-slate-200'
          : 'border-rose-300/25 bg-rose-300/12 text-rose-100';

  return (
    <div className="min-h-screen bg-[radial-gradient(circle_at_top_left,_rgba(34,211,238,0.22),_transparent_28%),radial-gradient(circle_at_bottom_right,_rgba(251,191,36,0.18),_transparent_24%),linear-gradient(180deg,_#04111f_0%,_#0f172a_55%,_#111827_100%)] text-slate-100">
      <div className="mx-auto flex min-h-screen w-full max-w-7xl flex-col gap-10 px-5 py-6 sm:px-8 lg:px-10">
        <header className="rounded-[2.2rem] border border-white/10 bg-slate-950/55 px-6 py-6 shadow-[0_25px_80px_rgba(2,6,23,0.45)] backdrop-blur md:px-8">
          <div className="flex flex-col gap-6 lg:flex-row lg:items-end lg:justify-between">
            <div>
              <p className="text-sm font-semibold uppercase tracking-[0.4em] text-cyan-300">AiWattCoach</p>
              <h1 className="mt-4 font-serif text-4xl leading-tight text-white md:text-5xl">
                AiWattCoach Control Center
              </h1>
              <p className="mt-4 max-w-2xl text-base leading-7 text-slate-300">
                A focused frontend shell for settings, diagnostics, and the next training workflows.
              </p>
            </div>

            <section className={`rounded-[1.6rem] border px-5 py-4 ${statusAccentClass}`}>
              <p className="text-xs uppercase tracking-[0.3em]">Backend status</p>
              <p className="mt-2 text-2xl font-semibold text-white">{backendStatus.state}</p>
              <p className="mt-1 text-sm text-slate-200">
                {backendStatus.health.service} · health {backendStatus.health.status} · ready{' '}
                {backendStatus.readiness.status}
              </p>
            </section>
          </div>

          <nav className="mt-8 flex flex-wrap gap-3">
            {navigationItems.map((item) => (
              <NavLink
                key={item.to}
                className={({ isActive }) =>
                  [
                    'rounded-full px-5 py-2.5 text-sm font-semibold transition',
                    isActive
                      ? 'bg-white text-slate-950 shadow-[0_10px_30px_rgba(255,255,255,0.16)]'
                      : 'bg-white/6 text-slate-200 hover:bg-white/12'
                  ].join(' ')
                }
                to={item.to}
              >
                {item.label}
              </NavLink>
            ))}
          </nav>
        </header>

        <main className="pb-10">
          <Outlet />
        </main>
      </div>
    </div>
  );
}
