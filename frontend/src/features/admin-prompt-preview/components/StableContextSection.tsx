import { useMemo, useState } from 'react';

import { MarkdownContent } from '../../../lib/markdown/MarkdownContent';
import { parseKeyValueLines, tryParseJson } from '../utils/parseContextLines';
import { DecodedPackedContext } from './DecodedPackedContext';
import { SectionCard } from './SectionCard';

type StableContextSectionProps = {
  rawText: string;
};

export function StableContextSection({ rawText }: StableContextSectionProps) {
  const [showRaw, setShowRaw] = useState(false);
  const [expanded, setExpanded] = useState(true);

  const parsed = useMemo(() => {
    const lines = parseKeyValueLines(rawText);
    const workoutSummary = tryParseJson<Record<string, unknown>>(lines.workout_summary ?? '');
    const selectedWorkout = tryParseJson<Record<string, unknown>>(lines.selected_workout ?? '');
    const packedJson = tryParseJson<Record<string, unknown>>(lines.training_context_stable ?? '');
    return {
      workoutId: workoutSummary ? String(workoutSummary.workoutId ?? '') : null,
      rpe: workoutSummary ? (workoutSummary.rpe != null ? Number(workoutSummary.rpe) : null) : null,
      workoutDate: selectedWorkout ? String(selectedWorkout.date ?? '') : null,
      workoutContext: lines.current_workout_context?.trim() ?? null,
      athleteSummary: lines.athlete_summary_text?.trim() ?? null,
      packedJson,
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

          {parsed.workoutContext && (
            <div className="whitespace-pre-wrap break-words rounded-xl border border-white/10 bg-white/[0.02] px-4 py-3 text-sm text-slate-300">
              {parsed.workoutContext}
            </div>
          )}

          {parsed.athleteSummary && (
            <details className="rounded-xl border border-white/10 bg-white/[0.02]">
              <summary className="cursor-pointer px-4 py-2 text-xs font-semibold uppercase tracking-[0.15em] text-slate-500">
                Athlete Summary
              </summary>
              <div className="whitespace-pre-wrap break-words border-t border-white/5 px-4 py-3 text-sm">
                <MarkdownContent>{parsed.athleteSummary}</MarkdownContent>
              </div>
            </details>
          )}

          {parsed.packedJson && (
            <div>
              <button
                type="button"
                onClick={() => setShowRaw(!showRaw)}
                className="mb-2 text-xs font-semibold uppercase tracking-wider text-slate-500 hover:text-slate-300"
              >
                {showRaw ? 'Decoded view' : 'Show raw JSON'}
              </button>
              {showRaw ? (
                <pre className="max-h-80 overflow-auto whitespace-pre-wrap break-words rounded-xl border border-white/10 bg-[#070b12] p-4 font-mono text-xs leading-5 text-slate-400">
                  {JSON.stringify(parsed.packedJson, null, 2)}
                </pre>
              ) : (
                <DecodedPackedContext label="Training Context" data={parsed.packedJson} />
              )}
            </div>
          )}
        </div>
      )}
    </SectionCard>
  );
}
