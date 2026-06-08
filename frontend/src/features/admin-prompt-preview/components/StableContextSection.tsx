import { useMemo, useState } from 'react';

import { parseStableContext } from '../utils/parseStableContext';
import { SectionCard } from './SectionCard';
import { StableContextBody } from './StableContextBody';

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
  const [expanded, setExpanded] = useState(true);
  const parsed = useMemo(() => parseStableContext(rawText), [rawText]);
  const packedLabel = parsed.packed ? packedContextLabel(parsed.packed.sourceKey) : 'Training Context';

  return (
    <SectionCard title={`Stable Context (${rawText.length} chars)`} expanded={expanded} onToggle={() => setExpanded(!expanded)}>
      {expanded ? <StableContextBody parsed={parsed} packedLabel={packedLabel} /> : null}
    </SectionCard>
  );
}
