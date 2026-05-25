import { NavLink, Outlet, useLocation } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import {
  Bell,
  Bot,
  Calendar,
  Flag,
  LayoutDashboard,
  ListChecks,
  Settings,
  ShieldCheck,
} from 'lucide-react';
import type { ComponentType } from 'react';

import { UserMenu } from '../features/auth/components/UserMenu';
import { useAuth } from '../features/auth/context/AuthProvider';
import { useSettings } from '../features/settings/context/SettingsContext';
import { useMediaQuery } from '../lib/useMediaQuery';

type AuthenticatedLayoutProps = {
  apiBaseUrl: string;
  backendStatus?: { state: string; health: { service: string; status: string }; readiness: { status: string } };
};

const PAGE_TITLE_KEYS: Record<string, string> = {
  '/app': 'appShell.pageTitles.dashboard',
  '/settings': 'appShell.pageTitles.settings',
  '/calendar': 'appShell.pageTitles.calendar',
  '/races': 'appShell.pageTitles.races',
  '/ai-coach': 'appShell.pageTitles.aiCoach',
  '/admin/task-scheduler': 'appShell.pageTitles.taskScheduler',
  '/admin/system-info': 'appShell.pageTitles.systemInfo',
};

export function AuthenticatedLayout({ apiBaseUrl }: AuthenticatedLayoutProps) {
  const { t } = useTranslation();
  const auth = useAuth();
  const location = useLocation();
  const settingsCtx = useSettings();
  const currentUser = auth.user;
  const isMobile = useMediaQuery('(max-width: 767px)');

  const pageTitleKey = PAGE_TITLE_KEYS[location.pathname];
  const pageTitle = pageTitleKey ? t(pageTitleKey) : 'WATTLY';
  const cycling = settingsCtx.settings?.cycling ?? null;
  const metricLabels = [
    cycling?.hrMaxBpm ? `HR ${cycling.hrMaxBpm}` : null,
    cycling?.ftpWatts ? `FTP ${cycling.ftpWatts}` : null,
    cycling?.vo2Max ? `VO2 ${cycling.vo2Max}` : null,
  ].filter((label): label is string => Boolean(label));

  return (
    <div className="flex min-h-dvh bg-[#0a0f1a] text-white">
      <aside className="fixed left-0 top-0 z-20 hidden h-screen w-56 flex-col border-r border-white/10 bg-[#070b12] md:flex">
        <div className="p-5">
          <p className="text-lg font-bold text-white tracking-wide">WATTLY</p>
          <p className="text-[10px] uppercase tracking-[0.25em] text-slate-500 mt-0.5">
            {t('appShell.brand.subtitle')}
          </p>
        </div>

        <nav className="mt-4 flex-1 px-3 space-y-1">
          <NavItem to="/app" icon={LayoutDashboard} label={t('nav.dashboard')} />
          <NavItem to="/calendar" icon={Calendar} label={t('nav.calendar')} />
          <NavItem to="/races" icon={Flag} label={t('nav.races')} />
          <NavItem to="/ai-coach" icon={Bot} label={t('nav.aiCoach')} />
          <NavItem to="/settings" icon={Settings} label={t('nav.settings')} />
          {currentUser && currentUser.roles.includes('admin') && (
            <>
              <NavItem to="/admin/task-scheduler" icon={ListChecks} label={t('nav.taskScheduler')} />
              <NavItem to="/admin/system-info" icon={ShieldCheck} label={t('nav.systemInfo')} />
            </>
          )}
        </nav>
      </aside>

      <div className="flex min-h-dvh flex-1 flex-col md:ml-56">
        <header className="sticky top-0 z-10 border-b border-white/10 bg-[#0a0f1a]/80 backdrop-blur">
          <div className="flex items-center justify-between gap-4 px-4 py-4 md:px-6">
            <div className="min-w-0">
              <p className="text-[10px] font-bold uppercase tracking-[0.24em] text-slate-500 md:hidden">
                WATTLY
              </p>
              <h1 className="truncate text-lg font-bold text-white md:text-xl">{pageTitle}</h1>
            </div>

            <div className="flex items-center gap-2 md:gap-4">
              <button
                className="text-slate-400 transition hover:text-slate-200"
                type="button"
                aria-label={t('appShell.notifications')}
              >
                <Bell size={18} />
              </button>

              <UserMenu apiBaseUrl={apiBaseUrl} compact={isMobile} />
            </div>
          </div>

          {metricLabels.length > 0 ? (
            <div className="hidden px-6 pb-4 md:block">
              <div className="flex flex-wrap items-center gap-2">
                {metricLabels.map((label) => <MetricPill key={label} label={label} />)}
              </div>
            </div>
          ) : null}
        </header>

        <main className="safe-bottom-nav-padding px-4 py-4 md:p-6 md:pb-6">
          <Outlet />
        </main>

        {isMobile ? <MobileBottomNav currentUserIsAdmin={Boolean(currentUser?.roles.includes('admin'))} /> : null}
      </div>
    </div>
  );
}

function MetricPill({ label }: { label: string }) {
  return (
    <span className="text-sm text-cyan-300 border border-cyan-400/40 rounded-full px-3 py-1 font-medium">
      {label}
    </span>
  );
}

function MobileBottomNav({ currentUserIsAdmin }: { currentUserIsAdmin: boolean }) {
  const { t } = useTranslation();

  return (
    <nav className="safe-side-padding safe-bottom fixed inset-x-0 bottom-0 z-20 md:hidden" aria-label={t('appShell.mobileNavigation')}>
      <div className="mx-auto flex max-w-lg items-center justify-between gap-2 rounded-[1.6rem] border border-white/10 bg-[#0b1018]/95 px-2 py-2 shadow-[0_25px_65px_rgba(0,0,0,0.45)] backdrop-blur-xl">
        <MobileNavItem to="/app" icon={LayoutDashboard} label={t('nav.dashboard')} />
        <MobileNavItem to="/calendar" icon={Calendar} label={t('nav.calendar')} />
        <MobileNavItem to="/ai-coach" icon={Bot} label={t('nav.aiCoach')} />
        <MobileNavItem to="/settings" icon={Settings} label={t('nav.settings')} />
        {currentUserIsAdmin ? <MobileNavItem to="/admin/task-scheduler" icon={ListChecks} label={t('nav.taskScheduler')} /> : null}
      </div>
    </nav>
  );
}

type NavItemProps = {
  to: string;
  icon: ComponentType<{ size?: number; className?: string }>;
  label: string;
};

function NavItem({ to, icon: Icon, label }: NavItemProps) {
  return (
    <NavLink
      key={to + label}
      to={to}
      className={({ isActive }) =>
        [
          'flex items-center gap-3 px-3 py-2.5 rounded-xl text-sm font-medium transition group',
          isActive
            ? 'bg-white/8 text-cyan-300 border-l-2 border-cyan-400 pl-[10px]'
            : 'text-slate-400 hover:text-slate-200 hover:bg-white/5',
        ].join(' ')
      }
    >
      <Icon size={17} className="shrink-0" />
      {label}
    </NavLink>
  );
}

function MobileNavItem({ to, icon: Icon, label }: NavItemProps) {
  return (
    <NavLink
      to={to}
      className={({ isActive }) => [
        'flex min-w-0 flex-1 flex-col items-center gap-1 rounded-2xl px-2 py-2 text-center text-[10px] font-bold uppercase tracking-[0.14em] transition',
        isActive
          ? 'bg-cyan-300/12 text-cyan-200'
          : 'text-slate-400 hover:bg-white/5 hover:text-slate-200',
      ].join(' ')}
    >
      <Icon size={17} className="shrink-0" />
      <span className="truncate">{label}</span>
    </NavLink>
  );
}
