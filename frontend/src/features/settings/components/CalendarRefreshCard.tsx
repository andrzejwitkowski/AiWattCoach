import { useState } from 'react';
import { AlertCircle, CheckCircle2, RefreshCw, RotateCcw } from 'lucide-react';

import { refreshCalendarView } from '../../calendar/api/calendar';
import { invalidateCalendarCache } from '../../calendar/hooks/useCalendarData';
import { AuthenticationError } from '../../../lib/httpClient';

type CalendarRefreshCardProps = {
  apiBaseUrl: string;
};

export function CalendarRefreshCard({ apiBaseUrl }: CalendarRefreshCardProps) {
  const [isRefreshing, setIsRefreshing] = useState(false);
  const [status, setStatus] = useState<{
    tone: 'success' | 'error';
    label: string;
    message: string;
  } | null>(null);

  const handleRefresh = async () => {
    if (isRefreshing) return;
    setIsRefreshing(true);
    setStatus(null);

    try {
      const result = await refreshCalendarView(apiBaseUrl);
      invalidateCalendarCache();
      setStatus({
        tone: 'success',
        label: 'Gotowe',
        message:
          result.rebuiltEntryCount > 0
            ? `Widok kalendarza zostal przegenerowany (${result.rebuiltEntryCount} wpisy, ${result.oldest} do ${result.newest}).`
            : `Widok kalendarza zostal przegenerowany (${result.oldest} do ${result.newest}).`,
      });
    } catch (error) {
      if (error instanceof AuthenticationError) {
        window.location.href = '/';
        return;
      }
      setStatus({
        tone: 'error',
        label: 'Nieudane',
        message: error instanceof Error ? error.message : 'Nie udalo sie przegenerowac widoku kalendarza.',
      });
    } finally {
      setIsRefreshing(false);
    }
  };

  const statusClasses =
    status?.tone === 'success'
      ? 'border-emerald-400/30 bg-emerald-500/10 text-emerald-200'
      : 'border-red-500/30 bg-red-500/10 text-red-200';
  const StatusIcon = status?.tone === 'success' ? CheckCircle2 : AlertCircle;

  return (
    <div className="rounded-2xl border border-white/10 bg-white/5 p-6 backdrop-blur">
      <div className="flex items-start gap-4">
        <div className="flex h-10 w-10 shrink-0 items-center justify-center rounded-xl bg-slate-800">
          <RotateCcw size={20} className="text-cyan-400" />
        </div>
        <div className="flex-1">
          <h2 className="text-xl font-bold text-white">Przegeneruj widok kalendarza</h2>
          <p className="mt-0.5 text-[10px] uppercase tracking-[0.2em] text-slate-500">
            Calendar View Rebuild
          </p>
        </div>
      </div>

      <p className="mt-4 text-sm leading-relaxed text-slate-300">
        Odbuduj zapisany widok kalendarza na podstawie aktualnych treningow, planow, zawodow i special days,
        takze w przyszlosci.
      </p>

      {status ? (
        <div
          className={`mt-4 rounded-xl border px-4 py-3 text-sm ${statusClasses}`}
          role={status.tone === 'error' ? 'alert' : 'status'}
          aria-live={status.tone === 'error' ? 'assertive' : 'polite'}
        >
          <div className="flex items-start gap-3">
            <StatusIcon size={16} className="mt-0.5 shrink-0" />
            <div>
              <p className="text-[11px] font-semibold uppercase tracking-wider">{status.label}</p>
              <p className="mt-1">{status.message}</p>
            </div>
          </div>
        </div>
      ) : null}

      <div className="mt-6 flex gap-3">
        <button
          type="button"
          className="flex flex-1 items-center justify-center gap-2 rounded-xl bg-cyan-400 py-3 text-sm font-semibold text-slate-950 transition hover:bg-cyan-300 disabled:cursor-not-allowed disabled:opacity-60"
          disabled={isRefreshing}
          onClick={() => {
            void handleRefresh();
          }}
        >
          <RefreshCw size={15} className={isRefreshing ? 'animate-spin' : undefined} />
          {isRefreshing ? 'Przegenerowywanie...' : 'Przegeneruj widok kalendarza'}
        </button>
      </div>
    </div>
  );
}
