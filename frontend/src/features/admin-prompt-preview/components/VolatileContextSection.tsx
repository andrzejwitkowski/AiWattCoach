import { useMemo, useState } from 'react';

import {
  parseContextSections,
  readJsonField,
  resolvePackedContextJson,
  VOLATILE_PACKED_CONTEXT_KEYS,
} from '../utils/contextSections';
import { tryParseJson } from '../utils/parseContextLines';
import { DecodedPackedContext } from './DecodedPackedContext';
import { SectionCard } from './SectionCard';

type VolatileContextSectionProps = {
  rawText: string;
};

function packedContextLabel(sourceKey: string): string {
  switch (sourceKey) {
    case 'training_plan_source_volatile':
      return 'Training Plan Volatile Context';
    case 'meso_cycle_source_volatile':
      return 'Meso Cycle Volatile Context';
    default:
      return 'Volatile Training Context';
  }
}

export function VolatileContextSection({ rawText }: VolatileContextSectionProps) {
  const [showRaw, setShowRaw] = useState(false);
  const [expanded, setExpanded] = useState(true);

  const parsed = useMemo(() => {
    const lines = parseContextSections(rawText);
    const timing = tryParseJson<Record<string, unknown>>(lines.conversation_timing ?? '');
    const calendarFocus = readJsonField(lines, 'calendar_focus');
    const packed = resolvePackedContextJson(lines, VOLATILE_PACKED_CONTEXT_KEYS);

    return {
      timing,
      latestUserMessageDatetime: lines.latest_user_message_datetime?.trim() ?? null,
      calendarFocus,
      packed,
    };
  }, [rawText]);

  return (
    <SectionCard title={`Volatile Context (${rawText.length} chars)`} expanded={expanded} onToggle={() => setExpanded(!expanded)}>
      {expanded && (
        <div className="space-y-4">
          {parsed.calendarFocus && (
            <div className="prompt-preview-text rounded-xl border border-white/10 bg-white/[0.02] px-4 py-3 text-sm">
              <div className="mb-1.5 text-xs font-semibold uppercase tracking-[0.15em] text-slate-500">
                Calendar Focus
              </div>
              <p className="text-slate-200">
                Kind: <span className="font-mono text-cyan-300">{String(parsed.calendarFocus.kind ?? '')}</span>
              </p>
            </div>
          )}

          {parsed.timing && (
            <div className="prompt-preview-text rounded-xl border border-white/10 bg-white/[0.02] px-4 py-3 text-sm">
              <div className="mb-1.5 text-xs font-semibold uppercase tracking-[0.15em] text-slate-500">Conversation Timing</div>
              <p className="text-slate-200">
                Current:{' '}
                <span className="font-mono text-cyan-300">{String(parsed.timing.currentConversationDatetime ?? '')}</span>
              </p>
              {parsed.latestUserMessageDatetime && (
                <p className="mt-1 text-slate-200">
                  Latest user msg:{' '}
                  <span className="font-mono text-amber-300">{parsed.latestUserMessageDatetime}</span>
                </p>
              )}
              <p className="mt-1 text-xs italic text-slate-500">{String(parsed.timing.instruction ?? '')}</p>
            </div>
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
