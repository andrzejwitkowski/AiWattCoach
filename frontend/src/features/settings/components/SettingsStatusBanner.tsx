import { AlertCircle, CheckCircle2, RefreshCw } from 'lucide-react';

export type SettingsStatusTone = 'neutral' | 'success' | 'error';

export type SettingsStatus = {
  tone: SettingsStatusTone;
  label: string;
  message: string;
};

function settingsStatusClasses(tone: SettingsStatusTone): string {
  if (tone === 'success') {
    return 'border-emerald-400/30 bg-emerald-500/10 text-emerald-200';
  }
  if (tone === 'error') {
    return 'border-red-500/30 bg-red-500/10 text-red-200';
  }
  return 'border-cyan-400/20 bg-cyan-400/10 text-cyan-100';
}

type SettingsStatusBannerProps = {
  status: SettingsStatus;
};

export function SettingsStatusBanner({ status }: SettingsStatusBannerProps) {
  const StatusIcon =
    status.tone === 'success' ? CheckCircle2 : status.tone === 'error' ? AlertCircle : RefreshCw;

  return (
    <div
      className={`mt-4 rounded-xl border px-4 py-3 text-sm ${settingsStatusClasses(status.tone)}`}
      role={status.tone === 'error' ? 'alert' : 'status'}
      aria-live={status.tone === 'error' ? 'assertive' : 'polite'}
    >
      <div className="flex items-start gap-3">
        <StatusIcon
          size={16}
          className={status.tone === 'neutral' ? 'mt-0.5 shrink-0 animate-spin' : 'mt-0.5 shrink-0'}
        />
        <div>
          <p className="text-[11px] font-semibold uppercase tracking-wider">{status.label}</p>
          <p className="mt-1">{status.message}</p>
        </div>
      </div>
    </div>
  );
}
