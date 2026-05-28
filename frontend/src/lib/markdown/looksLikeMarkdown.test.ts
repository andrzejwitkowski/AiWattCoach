import { describe, expect, it } from 'vitest';

import { looksLikeMarkdown } from './looksLikeMarkdown';

describe('looksLikeMarkdown', () => {
  it('returns false for plain prose summaries', () => {
    expect(
      looksLikeMarkdown('Strong aerobic control with only a small fade near the end.'),
    ).toBe(false);
  });

  it('returns false for empty or whitespace-only text', () => {
    expect(looksLikeMarkdown('')).toBe(false);
    expect(looksLikeMarkdown('   \n  ')).toBe(false);
  });

  it('detects headings', () => {
    expect(looksLikeMarkdown('### Workout Recap\n\nGood effort.')).toBe(true);
  });

  it('detects bold and list markers', () => {
    expect(looksLikeMarkdown('**Execution Quality:** solid\n- May 29: recovery')).toBe(true);
  });

  it('detects inline code', () => {
    expect(looksLikeMarkdown('Keep `FTP` steady through the block.')).toBe(true);
  });
});
