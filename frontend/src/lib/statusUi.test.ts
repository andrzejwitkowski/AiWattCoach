import { describe, expect, it } from 'vitest';

import {
  getReadinessMessage,
  getStatusPanelClass,
  getStatusToneClass
} from './statusUi';

describe('getStatusPanelClass', () => {
  it('maps every backend state to a panel class', () => {
    expect(getStatusPanelClass('online')).toBe('border-cyan-300/20 bg-cyan-300/10');
    expect(getStatusPanelClass('degraded')).toBe('border-amber-300/25 bg-amber-300/12');
    expect(getStatusPanelClass('loading')).toBe('border-slate-300/15 bg-slate-300/10');
    expect(getStatusPanelClass('offline')).toBe('border-rose-300/25 bg-rose-300/12');
  });

  it('falls back to the offline panel class for unexpected runtime values', () => {
    expect(getStatusPanelClass('mystery' as never)).toBe('border-rose-300/25 bg-rose-300/12');
  });
});

describe('getStatusToneClass', () => {
  it('maps every backend state to a tone class', () => {
    expect(getStatusToneClass('online')).toBe('border-cyan-300/20 bg-cyan-300/10 text-cyan-200');
    expect(getStatusToneClass('degraded')).toBe(
      'border-amber-300/25 bg-amber-300/12 text-amber-100'
    );
    expect(getStatusToneClass('loading')).toBe('border-slate-300/15 bg-slate-300/10 text-slate-200');
    expect(getStatusToneClass('offline')).toBe('border-rose-300/25 bg-rose-300/12 text-rose-100');
  });

  it('falls back to the offline tone class for unexpected runtime values', () => {
    expect(getStatusToneClass('mystery' as never)).toBe('border-rose-300/25 bg-rose-300/12 text-rose-100');
  });
});

describe('getReadinessMessage', () => {
  it('maps known degraded reason codes to readable copy', () => {
    expect(getReadinessMessage('degraded', 'mongo_unreachable')).toBe(
      'Backend cannot reach MongoDB right now.'
    );
  });

  it('falls back to the state message for unknown non-empty reasons', () => {
    expect(getReadinessMessage('degraded', 'cache_timeout')).toBe(
      'Backend is degraded; some features may be unavailable.'
    );
  });

  it('keeps the state message fallback when reason is null', () => {
    expect(getReadinessMessage('degraded', null)).toBe(
      'Backend is degraded; some features may be unavailable.'
    );
  });
});
