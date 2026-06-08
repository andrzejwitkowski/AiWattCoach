import {
  parseContextSections,
  readJsonField,
  resolvePackedContextJson,
  STABLE_PACKED_CONTEXT_KEYS,
} from './contextSections';
import { tryParseJson } from './parseContextLines';

export type ParsedStableContext = {
  workoutId: string | null;
  rpe: number | null;
  workoutDate: string | null;
  workoutContext: string | null;
  workoutRecap: string | null;
  athleteSummary: string | null;
  mesoWindowStart: string | null;
  mesoWindowEnd: string | null;
  mesoRoadmapGuidance: string | null;
  mesoRoadmap: Record<string, unknown> | null;
  savedAtEpochSeconds: string | null;
  calendarConversation: Record<string, unknown> | null;
  packed: ReturnType<typeof resolvePackedContextJson>;
};

function readWorkoutSummary(lines: Record<string, string>) {
  const workoutSummary = tryParseJson<Record<string, unknown>>(lines.workout_summary ?? '');
  if (!workoutSummary) {
    return { workoutId: null, rpe: null };
  }

  return {
    workoutId: String(workoutSummary.workoutId ?? ''),
    rpe: workoutSummary.rpe != null ? Number(workoutSummary.rpe) : null,
  };
}

function readSelectedWorkoutDate(lines: Record<string, string>) {
  const selectedWorkout = tryParseJson<Record<string, unknown>>(lines.selected_workout ?? '');
  return selectedWorkout ? String(selectedWorkout.date ?? '') : null;
}

export function parseStableContext(rawText: string): ParsedStableContext {
  const lines = parseContextSections(rawText);
  const { workoutId, rpe } = readWorkoutSummary(lines);

  return {
    workoutId,
    rpe,
    workoutDate: readSelectedWorkoutDate(lines),
    workoutContext: lines.current_workout_context?.trim() ?? null,
    workoutRecap: lines.current_workout_recap?.trim() ?? null,
    athleteSummary: lines.athlete_summary_text?.trim() ?? null,
    mesoWindowStart: lines.meso_cycle_window_start?.trim() ?? null,
    mesoWindowEnd: lines.meso_cycle_window_end?.trim() ?? null,
    mesoRoadmapGuidance: lines.meso_cycle_roadmap_guidance?.trim() ?? null,
    mesoRoadmap: readJsonField(lines, 'meso_cycle_roadmap'),
    savedAtEpochSeconds: lines.saved_at_epoch_seconds?.trim() ?? null,
    calendarConversation: readJsonField(lines, 'calendar_conversation'),
    packed: resolvePackedContextJson(lines, STABLE_PACKED_CONTEXT_KEYS),
  };
}
