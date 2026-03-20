import type { BackendStatusKind } from './api/system';

const readinessReasonMessages: Record<string, string> = {
  mongo_unreachable: 'Backend cannot reach MongoDB right now.'
};

export function getStatusPanelClass(state: BackendStatusKind): string {
  switch (state) {
    case 'online':
      return 'border-cyan-300/20 bg-cyan-300/10';
    case 'degraded':
      return 'border-amber-300/25 bg-amber-300/12';
    case 'loading':
      return 'border-slate-300/15 bg-slate-300/10';
    case 'offline':
      return 'border-rose-300/25 bg-rose-300/12';
  }
}

export function getStatusToneClass(state: BackendStatusKind): string {
  switch (state) {
    case 'online':
      return `${getStatusPanelClass(state)} text-cyan-200`;
    case 'degraded':
      return `${getStatusPanelClass(state)} text-amber-100`;
    case 'loading':
      return `${getStatusPanelClass(state)} text-slate-200`;
    case 'offline':
      return `${getStatusPanelClass(state)} text-rose-100`;
  }
}

export function getReadinessMessage(state: BackendStatusKind, reason: string | null): string {
  if (reason) {
    const message = readinessReasonMessages[reason];

    if (message) {
      return message;
    }
  }

  switch (state) {
    case 'online':
      return 'Backend reports ready for requests.';
    case 'degraded':
      return 'Backend is degraded; some features may be unavailable.';
    case 'loading':
      return 'Checking backend readiness.';
    case 'offline':
      return 'Backend is offline or unreachable.';
  }
}
