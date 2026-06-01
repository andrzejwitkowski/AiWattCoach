import { useState } from 'react';

import type { AdminPromptPreviewResponse } from '../types';
import { SectionCard } from './SectionCard';

type ConversationSectionProps = {
  conversation: AdminPromptPreviewResponse['request']['conversation'];
};

const ROLE_COLORS: Record<string, string> = {
  user: 'border-sky-500/30 bg-sky-500/5',
  assistant: 'border-emerald-500/30 bg-emerald-500/5',
  tool: 'border-amber-500/20 bg-amber-500/5',
  system: 'border-slate-500/30 bg-slate-500/5',
};

const ROLE_LABELS: Record<string, string> = {
  user: 'User',
  assistant: 'Coach',
  tool: 'Tool',
  system: 'System',
};

export function ConversationSection({ conversation }: ConversationSectionProps) {
  const [expanded, setExpanded] = useState(true);

  if (conversation.length === 0) return null;

  return (
    <SectionCard title={`Conversation (${conversation.length} messages)`} expanded={expanded} onToggle={() => setExpanded(!expanded)}>
      {expanded && (
        <div className="space-y-3">
          {conversation.map((msg, i) => {
            const role = String((msg as Record<string, unknown>).role ?? '');
            const content = String((msg as Record<string, unknown>).content ?? '');
            const toolCalls = (msg as Record<string, unknown>).tool_calls;
            return (
              <div key={i} className={`rounded-xl border px-4 py-3 ${ROLE_COLORS[role] ?? 'border-white/10 bg-white/[0.02]'}`}>
                <div className="mb-1 flex items-center gap-2">
                  <span className="text-xs font-semibold uppercase tracking-wider text-slate-500">
                    {ROLE_LABELS[role] ?? role}
                  </span>
                  <span className="text-[10px] text-slate-600">#{i}</span>
                </div>
                <div className="prompt-preview-text text-sm leading-6 text-slate-200">{content}</div>
                {Array.isArray(toolCalls) && toolCalls.length > 0 && (
                  <details className="mt-2">
                    <summary className="cursor-pointer text-xs text-slate-500">Tool calls ({toolCalls.length})</summary>
                    <pre className="prompt-preview-text mt-1 max-h-80 overflow-auto rounded-lg bg-[#070b12] p-2 font-mono text-xs text-slate-400">
                      {JSON.stringify(toolCalls, null, 2)}
                    </pre>
                  </details>
                )}
              </div>
            );
          })}
        </div>
      )}
    </SectionCard>
  );
}
