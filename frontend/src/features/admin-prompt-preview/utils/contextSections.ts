import { parseKeyValueLines, tryParseJson } from './parseContextLines';

export const STABLE_PACKED_CONTEXT_KEYS = [
  'training_context_stable',
  'training_plan_source_stable',
  'meso_cycle_source_stable',
] as const;

export const VOLATILE_PACKED_CONTEXT_KEYS = [
  'training_context_volatile',
  'training_plan_source_volatile',
  'meso_cycle_source_volatile',
] as const;

export type PackedContextSection = {
  sourceKey: string;
  data: Record<string, unknown>;
};

export function parseContextSections(rawText: string): Record<string, string> {
  return parseKeyValueLines(rawText);
}

export function resolvePackedContextJson(
  lines: Record<string, string>,
  keys: readonly string[],
): PackedContextSection | null {
  for (const key of keys) {
    const data = tryParseJson<Record<string, unknown>>(lines[key] ?? '');
    if (data) {
      return { sourceKey: key, data };
    }
  }
  return null;
}

export function readJsonField(
  lines: Record<string, string>,
  key: string,
): Record<string, unknown> | null {
  return tryParseJson<Record<string, unknown>>(lines[key] ?? '');
}
