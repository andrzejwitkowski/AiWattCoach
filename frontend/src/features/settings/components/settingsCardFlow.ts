import type { SettingsStatus } from './SettingsStatusBanner';

export function connectionErrorStatus(err: unknown, fallback: string): SettingsStatus {
  return {
    tone: 'error',
    label: 'FAILED',
    message: err instanceof Error ? err.message : fallback,
  };
}
