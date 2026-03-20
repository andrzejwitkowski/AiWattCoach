import { describe, expect, it } from 'vitest';

import { getReadinessMessage } from './statusUi';

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
