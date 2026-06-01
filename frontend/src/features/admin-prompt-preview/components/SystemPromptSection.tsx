import { useState } from 'react';

import { SectionCard } from './SectionCard';

type SystemPromptSectionProps = {
  systemPrompt: string;
};

export function SystemPromptSection({ systemPrompt }: SystemPromptSectionProps) {
  const [expanded, setExpanded] = useState(true);
  const lines = systemPrompt.split('\n');
  const toolIdx = lines.findIndex((l) => l.startsWith('Tool usage guidance:'));
  const introLines = toolIdx >= 0 ? lines.slice(0, toolIdx) : lines;
  const toolLines = toolIdx >= 0 ? lines.slice(toolIdx) : [];

  return (
    <SectionCard title={`System Prompt (${systemPrompt.length} chars)`} expanded={expanded} onToggle={() => setExpanded(!expanded)}>
      {expanded && (
        <div className="space-y-4 text-sm leading-6 text-slate-200">
          <div className="prompt-preview-text font-sans">{introLines.join('\n')}</div>
          {toolLines.length > 0 && (
            <details className="mt-3 rounded-xl border border-white/10 bg-white/5">
              <summary className="cursor-pointer px-3 py-2 text-xs font-semibold uppercase tracking-wider text-slate-400">
                Tool guidance ({toolLines.length} lines)
              </summary>
              <div className="prompt-preview-text px-3 pb-3 pt-1 font-sans text-slate-300">
                {toolLines.join('\n')}
              </div>
            </details>
          )}
        </div>
      )}
    </SectionCard>
  );
}
