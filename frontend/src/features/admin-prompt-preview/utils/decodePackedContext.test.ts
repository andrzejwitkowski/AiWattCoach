import { describe, expect, it } from 'vitest';

import { decodeShortKey, formatSeconds, formatSegmentTriplets } from './decodePackedContext';

describe('decodeShortKey', () => {
  it('maps known short keys to human labels', () => {
    expect(decodeShortKey('ctl')).toBe('CTL');
    expect(decodeShortKey('fnm')).toBe('Full Name');
    expect(decodeShortKey('tss')).toBe('TSS');
    expect(decodeShortKey('ps')).toBe('Power segments [minW,maxW,durSec]');
    expect(decodeShortKey('cs')).toBe('Cadence segments [minRPM,maxRPM,durSec]');
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

describe('formatSegmentTriplets', () => {
  it('formats steady and ranged segments', () => {
    expect(formatSegmentTriplets([[220, 220, 180], [240, 260, 120]], 'W')).toEqual([
      '220 W · 3m',
      '240–260 W · 2m',
    ]);
    expect(formatSegmentTriplets([[87, 87, 300]], 'RPM')).toEqual(['87 RPM · 5m']);
  });
});
