import { NavLink, Outlet, useLocation } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import {
  Bell,
  Bot,
  Calendar,
  Flag,
  LayoutDashboard,
  Settings,
  ShieldCheck,
} from 'lucide-react';
import type { ComponentType } from 'react';

import { UserMenu } from '../features/auth/components/UserMenu';
import { useAuth } from '../features/auth/context/AuthProvider';
import { useSettings } from '../features/settings/context/SettingsContext';

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
  '/admin/system-info': 'appShell.pageTitles.systemInfo',
};

export function AuthenticatedLayout({ apiBaseUrl }: AuthenticatedLayoutProps) {
  const { t } = useTranslation();
  const auth = useAuth();
  const location = useLocation();
  const settingsCtx = useSettings();
  const currentUser = auth.user;

  const pageTitleKey = PAGE_TITLE_KEYS[location.pathname];
  const pageTitle = pageTitleKey ? t(pageTitleKey) : 'WATTLY';
  const cycling = settingsCtx.settings?.cycling ?? null;

  return (
    <div className="flex min-h-screen bg-[#0a0f1a]">
      <aside className="fixed left-0 top-0 h-screen w-56 flex flex-col bg-[#070b12] border-r border-white/10 z-20">
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
            <NavItem to="/admin/system-info" icon={ShieldCheck} label={t('nav.systemInfo')} />
          )}
        </nav>
      </aside>

      <div className="ml-56 flex-1 flex flex-col min-h-screen">
        <header className="sticky top-0 z-10 flex items-center justify-between px-6 py-4 bg-[#0a0f1a]/80 backdrop-blur border-b border-white/10">
          <h1 className="text-xl font-bold text-white">{pageTitle}</h1>

          <div className="flex items-center gap-4">
            <div className="flex items-center gap-2">
              {cycling?.hrMaxBpm && <MetricPill label={`HR ${cycling.hrMaxBpm}`} />}
              {cycling?.ftpWatts && <MetricPill label={`FTP ${cycling.ftpWatts}`} />}
              {cycling?.vo2Max && <MetricPill label={`VO2 ${cycling.vo2Max}`} />}
            </div>

            <button
              className="text-slate-400 hover:text-slate-200 transition"
              type="button"
              aria-label={t('appShell.notifications')}
            >
              <Bell size={18} />
            </button>

            <UserMenu apiBaseUrl={apiBaseUrl} />
          </div>
        </header>

        <main className="p-6">
          <Outlet />
        </main>
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
