import { CheckCircle2, ExternalLink, Link2 } from 'lucide-react';

import { buildWahooConnectUrl } from '../../auth/api/auth';
import type { UserSettingsResponse } from '../types';

type WahooCardProps = {
  settings: UserSettingsResponse;
  apiBaseUrl: string;
};

function currentSettingsReturnTo() {
  if (typeof window === 'undefined') {
    return '/settings';
  }

  const { pathname, search, hash } = window.location;
  if (!pathname.startsWith('/settings')) {
    return '/settings';
  }

  return `${pathname}${search}${hash}`;
}

function formatExpiry(epochSeconds: number | null) {
  if (epochSeconds == null) {
    return 'Unknown';
  }

  const value = new Date(epochSeconds * 1000);
  if (Number.isNaN(value.getTime())) {
    return 'Unknown';
  }

  return value.toLocaleString();
}

function statusValue(value: boolean) {
  return value ? 'Saved' : 'Missing';
}

export function WahooCard({ settings, apiBaseUrl }: WahooCardProps) {
  const { wahoo } = settings;

  const handleConnect = () => {
    if (!wahoo.available) {
      return;
    }

    window.location.assign(buildWahooConnectUrl(apiBaseUrl, currentSettingsReturnTo()));
  };

  return (
    <div className="rounded-2xl border border-white/10 bg-white/5 p-6 backdrop-blur">
      <div className="flex items-start gap-4">
        <div className="flex h-10 w-10 shrink-0 items-center justify-center rounded-xl bg-violet-400/20 text-violet-300">
          <Link2 size={20} />
        </div>
        <div className="flex-1">
          <h2 className="text-xl font-bold text-white">Wahoo Cloud</h2>
          <p className="mt-0.5 text-[10px] uppercase tracking-[0.2em] text-slate-500">
            OAuth Connection
          </p>
        </div>
        {wahoo.connected && (
          <span className="rounded-full bg-emerald-400/20 px-2 py-0.5 text-[10px] font-semibold uppercase tracking-wider text-emerald-400">
            Connected
          </span>
        )}
      </div>

      <p className="mt-4 text-sm leading-relaxed text-slate-300">
        Connect Wahoo Cloud to store OAuth credentials for future sync features. This flow only manages
        account connection and token refresh.
      </p>

      {!wahoo.available && (
        <div className="mt-4 rounded-xl border border-amber-400/20 bg-amber-400/10 px-4 py-3 text-sm text-amber-100">
          Wahoo OAuth is not configured on this server yet.
        </div>
      )}

      <div className="mt-5 grid gap-3 sm:grid-cols-3">
        <div className="rounded-xl border border-white/10 bg-slate-950/40 px-4 py-3">
          <p className="text-[10px] uppercase tracking-[0.18em] text-slate-500">Access Token</p>
          <p className="mt-2 text-sm font-medium text-white">{statusValue(wahoo.accessTokenSet)}</p>
        </div>
        <div className="rounded-xl border border-white/10 bg-slate-950/40 px-4 py-3">
          <p className="text-[10px] uppercase tracking-[0.18em] text-slate-500">Refresh Token</p>
          <p className="mt-2 text-sm font-medium text-white">{statusValue(wahoo.refreshTokenSet)}</p>
        </div>
        <div className="rounded-xl border border-white/10 bg-slate-950/40 px-4 py-3">
          <p className="text-[10px] uppercase tracking-[0.18em] text-slate-500">Expires</p>
          <p className="mt-2 text-sm font-medium text-white">{formatExpiry(wahoo.expiresAtEpochSeconds)}</p>
        </div>
      </div>

      {wahoo.connected && (
        <div className="mt-4 rounded-xl border border-emerald-400/20 bg-emerald-500/10 px-4 py-3 text-sm text-emerald-100">
          <div className="flex items-start gap-3">
            <CheckCircle2 size={16} className="mt-0.5 shrink-0" />
            <div>
              <p className="font-semibold uppercase tracking-wider text-[11px]">Ready</p>
              <p className="mt-1">Wahoo tokens are stored in your settings and can be refreshed when needed.</p>
            </div>
          </div>
        </div>
      )}

      <button
        type="button"
        className="mt-6 flex w-full items-center justify-center gap-2 rounded-xl bg-violet-400 py-3 text-sm font-semibold text-slate-950 transition hover:bg-violet-300 disabled:cursor-not-allowed disabled:opacity-60"
        onClick={handleConnect}
        disabled={!wahoo.available}
      >
        <ExternalLink size={15} />
        {wahoo.connected ? 'Reconnect Wahoo' : 'Connect Wahoo'}
      </button>
    </div>
  );
}
