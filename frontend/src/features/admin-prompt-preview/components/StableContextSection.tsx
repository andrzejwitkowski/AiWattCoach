import { useMemo, useState } from 'react';

import { MarkdownContent } from '../../../lib/markdown/MarkdownContent';
import { tryParseJson } from '../utils/parseContextLines';
import { SectionCard } from './SystemPromptSection';
import { DecodedPackedContext } from './DecodedPackedContext';

type StableContextSectionProps = {
  rawText: string;
};

export function StableContextSection({ rawText }: StableContextSectionProps) {
  const [showRaw, setShowRaw] = useState(false);
  const [expanded, setExpanded] = useState(true);

  const parsed = useMemo(() => parseStableContext(rawText), [rawText]);

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
            <div className="rounded-xl border border-white/10 bg-white/[0.02] px-4 py-3 text-sm text-slate-300">
              {parsed.workoutContext}
            </div>
          )}

          {parsed.athleteSummary && (
            <details className="rounded-xl border border-white/10 bg-white/[0.02]">
              <summary className="cursor-pointer px-4 py-2 text-xs font-semibold uppercase tracking-[0.15em] text-slate-500">
                Athlete Summary
              </summary>
              <div className="border-t border-white/5 px-4 py-3 text-sm">
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

function parseStableContext(text: string) {
  const result: {
    workoutId: string | null;
    rpe: number | null;
    workoutDate: string | null;
    workoutContext: string | null;
    athleteSummary: string | null;
    packedJson: Record<string, unknown> | null;
  } = {
    workoutId: null,
    rpe: null,
    workoutDate: null,
    workoutContext: null,
    athleteSummary: null,
    packedJson: null,
  };

  const lines = text.split('\n');
  let currentKey: string | null = null;
  let currentValue = '';

  for (const line of lines) {
    if (currentKey === null) {
      const eqIdx = line.indexOf('=');
      if (eqIdx < 0) continue;
      currentKey = line.slice(0, eqIdx);
      currentValue = line.slice(eqIdx + 1);
    } else {
      currentValue += '\n' + line;
    }

    if (currentKey === 'workout_summary') {
      const parsed = tryParseJson<Record<string, unknown>>(currentValue);
      if (parsed) {
        result.workoutId = String(parsed.workoutId ?? '');
        result.rpe = parsed.rpe != null ? Number(parsed.rpe) : null;
      }
      currentKey = null;
      currentValue = '';
    } else if (currentKey === 'selected_workout') {
      const parsed = tryParseJson<Record<string, unknown>>(currentValue);
      if (parsed) result.workoutDate = String(parsed.date ?? '');
      currentKey = null;
      currentValue = '';
    } else if (currentKey === 'current_workout_context') {
      result.workoutContext = currentValue.trim();
      currentKey = null;
      currentValue = '';
    } else if (currentKey === 'athlete_summary_text') {
      result.athleteSummary = (result.athleteSummary ?? '') + currentValue;
      currentKey = null;
      currentValue = '';
    } else if (currentKey === 'training_context_stable') {
      const parsed = tryParseJson<Record<string, unknown>>(currentValue);
      if (parsed) result.packedJson = parsed;
      currentKey = null;
      currentValue = '';
    }
  }

  if (currentKey === 'athlete_summary_text') {
    result.athleteSummary = (result.athleteSummary ?? '') + currentValue;
  }

  return result;
}
