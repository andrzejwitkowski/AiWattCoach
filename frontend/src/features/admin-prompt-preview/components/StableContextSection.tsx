import { useMemo, useState } from 'react';

import { MarkdownContent } from '../../../lib/markdown/MarkdownContent';
import {
  parseContextSections,
  readJsonField,
  resolvePackedContextJson,
  STABLE_PACKED_CONTEXT_KEYS,
} from '../utils/contextSections';
import { tryParseJson } from '../utils/parseContextLines';
import { DecodedPackedContext } from './DecodedPackedContext';
import { SectionCard } from './SectionCard';

type StableContextSectionProps = {
  rawText: string;
};

function packedContextLabel(sourceKey: string): string {
  switch (sourceKey) {
    case 'training_plan_source_stable':
      return 'Training Plan Context';
    case 'meso_cycle_source_stable':
      return 'Meso Cycle Context';
    default:
      return 'Training Context';
  }
}

export function StableContextSection({ rawText }: StableContextSectionProps) {
  const [showRaw, setShowRaw] = useState(false);
  const [expanded, setExpanded] = useState(true);

  const parsed = useMemo(() => {
    const lines = parseContextSections(rawText);
    const workoutSummary = tryParseJson<Record<string, unknown>>(lines.workout_summary ?? '');
    const selectedWorkout = tryParseJson<Record<string, unknown>>(lines.selected_workout ?? '');
    const calendarConversation = readJsonField(lines, 'calendar_conversation');
    const packed = resolvePackedContextJson(lines, STABLE_PACKED_CONTEXT_KEYS);

    return {
      workoutId: workoutSummary ? String(workoutSummary.workoutId ?? '') : null,
      rpe: workoutSummary ? (workoutSummary.rpe != null ? Number(workoutSummary.rpe) : null) : null,
      workoutDate: selectedWorkout ? String(selectedWorkout.date ?? '') : null,
      workoutContext: lines.current_workout_context?.trim() ?? null,
      workoutRecap: lines.current_workout_recap?.trim() ?? null,
      athleteSummary: lines.athlete_summary_text?.trim() ?? null,
      mesoWindowStart: lines.meso_cycle_window_start?.trim() ?? null,
      mesoWindowEnd: lines.meso_cycle_window_end?.trim() ?? null,
      mesoRoadmapGuidance: lines.meso_cycle_roadmap_guidance?.trim() ?? null,
      mesoRoadmap: readJsonField(lines, 'meso_cycle_roadmap'),
      savedAtEpochSeconds: lines.saved_at_epoch_seconds?.trim() ?? null,
      calendarConversation,
      packed,
    };
  }, [rawText]);

  return (
    <SectionCard title={`Stable Context (${rawText.length} chars)`} expanded={expanded} onToggle={() => setExpanded(!expanded)}>
      {expanded && (
        <div className="space-y-4">
          {parsed.workoutId && parsed.rpe != null && (
            <div className="flex flex-wrap gap-4 text-sm">
              <span className="text-slate-400">
                Workout: <span className="text-slate-200">{parsed.workoutId}</span>
              </span>
              <span className="text-slate-400">
                RPE: <span className="text-slate-200">{parsed.rpe}</span>
              </span>
              {parsed.workoutDate && (
                <span className="text-slate-400">
                  Date: <span className="text-slate-200">{parsed.workoutDate}</span>
                </span>
              )}
            </div>
          )}

          {parsed.calendarConversation && (
            <div className="flex flex-wrap gap-4 text-sm">
              <span className="text-slate-400">
                Conversation:{' '}
                <span className="font-mono text-slate-200">
                  {String(parsed.calendarConversation.conversationId ?? '')}
                </span>
              </span>
              <span className="text-slate-400">
                Surface: <span className="text-slate-200">{String(parsed.calendarConversation.surface ?? '')}</span>
              </span>
              <span className="text-slate-400">
                Focus: <span className="text-slate-200">{String(parsed.calendarConversation.focus ?? '')}</span>
              </span>
            </div>
          )}

          {(parsed.mesoWindowStart || parsed.mesoWindowEnd) && (
            <div className="flex flex-wrap gap-4 text-sm">
              {parsed.mesoWindowStart && (
                <span className="text-slate-400">
                  Meso start: <span className="text-slate-200">{parsed.mesoWindowStart}</span>
                </span>
              )}
              {parsed.mesoWindowEnd && (
                <span className="text-slate-400">
                  Meso end: <span className="text-slate-200">{parsed.mesoWindowEnd}</span>
                </span>
              )}
            </div>
          )}

          {parsed.savedAtEpochSeconds && (
            <div className="text-sm text-slate-400">
              Saved at epoch: <span className="font-mono text-slate-200">{parsed.savedAtEpochSeconds}</span>
            </div>
          )}

          {parsed.workoutContext && (
            <div className="prompt-preview-text rounded-xl border border-white/10 bg-white/[0.02] px-4 py-3 text-sm text-slate-300">
              {parsed.workoutContext}
            </div>
          )}

          {parsed.workoutRecap && (
            <details className="rounded-xl border border-white/10 bg-white/[0.02]">
              <summary className="cursor-pointer px-4 py-2 text-xs font-semibold uppercase tracking-[0.15em] text-slate-500">
                Current Workout Recap
              </summary>
              <div className="prompt-preview-text border-t border-white/5 px-4 py-3 text-sm">
                <MarkdownContent>{parsed.workoutRecap}</MarkdownContent>
              </div>
            </details>
          )}

          {parsed.athleteSummary && (
            <details className="rounded-xl border border-white/10 bg-white/[0.02]">
              <summary className="cursor-pointer px-4 py-2 text-xs font-semibold uppercase tracking-[0.15em] text-slate-500">
                Athlete Summary
              </summary>
              <div className="prompt-preview-text border-t border-white/5 px-4 py-3 text-sm">
                <MarkdownContent>{parsed.athleteSummary}</MarkdownContent>
              </div>
            </details>
          )}

          {parsed.mesoRoadmapGuidance && (
            <div className="rounded-xl border border-amber-500/20 bg-amber-500/5 px-4 py-3 text-sm text-amber-100/90">
              <div className="mb-1 text-xs font-semibold uppercase tracking-[0.15em] text-amber-200/70">
                Meso Cycle Roadmap (predicted)
              </div>
              {parsed.mesoRoadmapGuidance}
            </div>
          )}

          {parsed.mesoRoadmap && (
            <details className="rounded-xl border border-white/10 bg-white/[0.02]" open>
              <summary className="cursor-pointer px-4 py-2 text-xs font-semibold uppercase tracking-[0.15em] text-slate-500">
                Meso Cycle Roadmap Days
              </summary>
              <div className="border-t border-white/5 px-4 py-3">
                <DecodedPackedContext label="Meso Cycle Roadmap" data={parsed.mesoRoadmap} />
              </div>
            </details>
          )}

          {parsed.packed && (
            <div>
              <button
                type="button"
                onClick={() => setShowRaw(!showRaw)}
                className="mb-2 text-xs font-semibold uppercase tracking-wider text-slate-500 hover:text-slate-300"
              >
                {showRaw ? 'Decoded view' : 'Show raw JSON'}
              </button>
              {showRaw ? (
                <pre className="prompt-preview-text max-h-80 overflow-auto rounded-xl border border-white/10 bg-[#070b12] p-4 font-mono text-xs leading-5 text-slate-400">
                  {JSON.stringify(parsed.packed.data, null, 2)}
                </pre>
              ) : (
                <DecodedPackedContext
                  label={packedContextLabel(parsed.packed.sourceKey)}
                  data={parsed.packed.data}
                />
              )}
            </div>
          )}
        </div>
      )}
    </SectionCard>
  );
}
