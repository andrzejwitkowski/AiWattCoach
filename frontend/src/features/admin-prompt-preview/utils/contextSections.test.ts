import { describe, expect, it } from 'vitest';

import {
  parseContextSections,
  resolvePackedContextJson,
  STABLE_PACKED_CONTEXT_KEYS,
  VOLATILE_PACKED_CONTEXT_KEYS,
} from './contextSections';

describe('contextSections', () => {
  it('resolves training_context_stable packed json', () => {
    const lines = parseContextSections(
      'workout_summary={"workoutId":"w1","rpe":7}\ntraining_context_stable={"ctl":50}',
    );

    const packed = resolvePackedContextJson(lines, STABLE_PACKED_CONTEXT_KEYS);
    expect(packed?.sourceKey).toBe('training_context_stable');
    expect(packed?.data).toEqual({ ctl: 50 });
  });

  it('falls back to meso_cycle_source_stable when training keys are absent', () => {
    const lines = parseContextSections(
      'meso_cycle_window_start=2026-06-20\nmeso_cycle_source_stable={"meso":true}',
    );

    const packed = resolvePackedContextJson(lines, STABLE_PACKED_CONTEXT_KEYS);
    expect(packed?.sourceKey).toBe('meso_cycle_source_stable');
    expect(packed?.data).toEqual({ meso: true });
  });

  it('resolves training_plan_source_volatile packed json', () => {
    const lines = parseContextSections(
      'conversation_timing={"currentConversationDatetime":"2026-05-01T23:59:59Z"}\ntraining_plan_source_volatile={"pd":[]}',
    );

    const packed = resolvePackedContextJson(lines, VOLATILE_PACKED_CONTEXT_KEYS);
    expect(packed?.sourceKey).toBe('training_plan_source_volatile');
    expect(packed?.data).toEqual({ pd: [] });
  });

  it('parses current_workout_recap and calendar metadata sections', () => {
    const stable = parseContextSections(
      'calendar_conversation={"conversationId":"c1","surface":"calendar","focus":"overview"}\ncurrent_workout_recap=Strong finish\ntraining_context_stable={"ok":1}',
    );
    const volatile = parseContextSections(
      'calendar_focus={"kind":"overview"}\ntraining_context_volatile={"ud":[]}',
    );

    expect(stable.current_workout_recap).toBe('Strong finish');
    expect(stable.calendar_conversation).toBe(
      '{"conversationId":"c1","surface":"calendar","focus":"overview"}',
    );
    expect(volatile.calendar_focus).toBe('{"kind":"overview"}');
  });
});
