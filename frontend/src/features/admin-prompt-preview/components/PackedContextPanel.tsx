import { useState } from 'react';

import { DecodedPackedContext } from './DecodedPackedContext';

type PackedContextPanelProps = {
  label: string;
  data: Record<string, unknown>;
};

export function PackedContextPanel({ label, data }: PackedContextPanelProps) {
  const [showRaw, setShowRaw] = useState(false);

  return (
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
          {JSON.stringify(data, null, 2)}
        </pre>
      ) : (
        <DecodedPackedContext label={label} data={data} />
      )}
    </div>
  );
}
