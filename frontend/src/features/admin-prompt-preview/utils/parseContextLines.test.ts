import { describe, expect, it } from 'vitest';

import { parseKeyValueLines, tryParseJson } from './parseContextLines';

describe('parseKeyValueLines', () => {
  it('parses simple key=value pairs', () => {
    const result = parseKeyValueLines('a=1\nb=2');
    expect(result).toEqual({ a: '1', b: '2' });
  });

  it('parses JSON values spanning newlines', () => {
    const text = 'x={"a":1}\ny=2';
    const result = parseKeyValueLines(text);
    expect(result).toEqual({ x: '{"a":1}', y: '2' });
  });

  it('parses the athlete_summary_text multi-line value', () => {
    const text = 'athlete_summary_text=**bold**\nmore text';
    const result = parseKeyValueLines(text);
    expect(result).toEqual({ athlete_summary_text: '**bold**\nmore text' });
  });

  it('parses deeply nested JSON multi-line', () => {
    const text = 'ctx={"a":{"b":1}}\nnext=2';
    const result = parseKeyValueLines(text);
    expect(result).toEqual({ ctx: '{"a":{"b":1}}', next: '2' });
  });
});

describe('tryParseJson', () => {
  it('parses valid JSON', () => {
    expect(tryParseJson('{"a":1}')).toEqual({ a: 1 });
  });

  it('returns null for invalid JSON', () => {
    expect(tryParseJson('not json')).toBeNull();
  });
});
