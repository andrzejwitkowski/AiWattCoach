import { useState } from 'react';

import type { AdminPromptPreviewResponse } from '../types';
import { SectionCard } from './SectionCard';

type ProviderMessagesSectionProps = {
  messages: AdminPromptPreviewResponse['providerMessages'];
};

const ROLE_LABELS: Record<string, string> = {
  system: 'System',
  user: 'User',
  assistant: 'Assistant',
  tool: 'Tool',
};

export function ProviderMessagesSection({ messages }: ProviderMessagesSectionProps) {
  const [expanded, setExpanded] = useState(false);

  if (messages.length === 0) return null;

  return (
    <SectionCard title={`Provider Messages (${messages.length})`} expanded={expanded} onToggle={() => setExpanded(!expanded)}>
      {expanded && (
        <div className="space-y-3">
          {messages.map((msg, i) => (
            <div
              key={i}
              className={`rounded-xl border px-4 py-3 ${
                msg.role === 'system'
                  ? 'border-slate-500/20 bg-slate-500/5'
                  : msg.role === 'assistant'
                    ? 'border-emerald-500/20 bg-emerald-500/5'
                    : 'border-white/10 bg-white/[0.02]'
              }`}
            >
              <div className="mb-1 text-xs font-semibold uppercase tracking-wider text-slate-500">
                {ROLE_LABELS[msg.role] ?? msg.role}
              </div>
              <pre className="whitespace-pre-wrap break-words font-mono text-xs leading-5 text-slate-300">
                {msg.content.length > 5000 ? msg.content.slice(0, 5000) + '\n… (truncated)' : msg.content}
              </pre>
            </div>
          ))}
        </div>
      )}
    </SectionCard>
  );
}
