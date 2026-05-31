import { useMemo, useState } from 'react';

import { tryParseJson } from '../utils/parseContextLines';
import { DecodedPackedContext } from './DecodedPackedContext';
import { SectionCard } from './SystemPromptSection';

type VolatileContextSectionProps = {
  rawText: string;
};

export function VolatileContextSection({ rawText }: VolatileContextSectionProps) {
  const [showRaw, setShowRaw] = useState(false);
  const [expanded, setExpanded] = useState(true);

  const parsed = useMemo(() => parseVolatileContext(rawText), [rawText]);

  return (
    <SectionCard title={`Volatile Context (${rawText.length} chars)`} expanded={expanded} onToggle={() => setExpanded(!expanded)}>
      {expanded && (
        <div className="space-y-4">
          {parsed.timing && (
            <div className="rounded-xl border border-white/10 bg-white/[0.02] px-4 py-3 text-sm">
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
                <DecodedPackedContext label="Volatile Training Context" data={parsed.packedJson} />
              )}
            </div>
          )}
        </div>
      )}
    </SectionCard>
  );
}

function parseVolatileContext(text: string) {
  let timing: Record<string, unknown> | null = null;
  let latestUserMessageDatetime: string | null = null;
  let packedJson: Record<string, unknown> | null = null;

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

    if (currentKey === 'conversation_timing') {
      timing = tryParseJson<Record<string, unknown>>(currentValue);
      currentKey = null;
      currentValue = '';
    } else if (currentKey === 'latest_user_message_datetime') {
      latestUserMessageDatetime = currentValue.trim();
      currentKey = null;
      currentValue = '';
    } else if (currentKey === 'training_context_volatile') {
      packedJson = tryParseJson<Record<string, unknown>>(currentValue);
      currentKey = null;
      currentValue = '';
    }
  }

  return { timing, latestUserMessageDatetime, packedJson };
}
