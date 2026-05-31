import { describe, expect, it } from 'vitest';

import { decodeShortKey, formatSeconds } from './decodePackedContext';

describe('decodeShortKey', () => {
  it('maps known short keys to human labels', () => {
    expect(decodeShortKey('ctl')).toBe('CTL');
    expect(decodeShortKey('fnm')).toBe('Full Name');
    expect(decodeShortKey('tss')).toBe('TSS');
  });

  it('returns the key as-is when unknown', () => {
    expect(decodeShortKey('zzz')).toBe('zzz');
    expect(decodeShortKey('')).toBe('');
  });
});

describe('formatSeconds', () => {
  it('formats hours and minutes', () => {
    expect(formatSeconds(7722)).toBe('2h 8m');
    expect(formatSeconds(3600)).toBe('1h 0m');
  });

  it('formats minutes only', () => {
    expect(formatSeconds(1842)).toBe('30m');
    expect(formatSeconds(60)).toBe('1m');
  });
});
