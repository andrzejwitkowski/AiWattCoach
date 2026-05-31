import { useState } from 'react';

import type { AdminPromptPreviewResponse } from '../types';
import { SectionCard } from './SystemPromptSection';

type ToolsSectionProps = {
  tools: AdminPromptPreviewResponse['request']['tools'];
  toolChoice: unknown;
};

export function ToolsSection({ tools, toolChoice }: ToolsSectionProps) {
  const [expanded, setExpanded] = useState(false);

  if (tools.length === 0) return null;

  return (
    <SectionCard title={`Tools (${tools.length})`} expanded={expanded} onToggle={() => setExpanded(!expanded)}>
      {expanded && (
        <div className="space-y-3">
          <div className="text-xs text-slate-500">
            Tool choice: <span className="font-mono text-slate-300">{JSON.stringify(toolChoice)}</span>
          </div>
          {tools.map((tool, i) => (
            <div key={i} className="rounded-xl border border-white/10 bg-white/[0.02] px-4 py-3">
              <div className="mb-1 font-mono text-sm font-semibold text-cyan-300">{tool.name}</div>
              {tool.description && (
                <div className="whitespace-pre-wrap break-words text-xs leading-5 text-slate-400">{tool.description}</div>
              )}
            </div>
          ))}
        </div>
      )}
    </SectionCard>
  );
}
