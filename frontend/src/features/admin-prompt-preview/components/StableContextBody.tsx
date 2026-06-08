import { MarkdownContent } from '../../../lib/markdown/MarkdownContent';
import type { ParsedStableContext } from '../utils/parseStableContext';
import { DecodedPackedContext } from './DecodedPackedContext';
import { PackedContextPanel } from './PackedContextPanel';

type StableContextBodyProps = {
  parsed: ParsedStableContext;
  packedLabel: string;
};

export function StableContextBody({ parsed, packedLabel }: StableContextBodyProps) {
  return (
    <div className="space-y-4">
      <StableWorkoutSummary parsed={parsed} />
      <StableConversationSummary conversation={parsed.calendarConversation} />
      <StableMesoWindow windowStart={parsed.mesoWindowStart} windowEnd={parsed.mesoWindowEnd} />
      <StableSavedAt savedAtEpochSeconds={parsed.savedAtEpochSeconds} />
      <StableTextBlock text={parsed.workoutContext} />
      <StableMarkdownDetails title="Current Workout Recap" text={parsed.workoutRecap} />
      <StableMarkdownDetails title="Athlete Summary" text={parsed.athleteSummary} />
      <StableMesoRoadmap guidance={parsed.mesoRoadmapGuidance} roadmap={parsed.mesoRoadmap} />
      {parsed.packed ? <PackedContextPanel label={packedLabel} data={parsed.packed.data} /> : null}
    </div>
  );
}

function StableWorkoutSummary({ parsed }: { parsed: ParsedStableContext }) {
  if (!parsed.workoutId || parsed.rpe == null) {
    return null;
  }

  return (
    <div className="flex flex-wrap gap-4 text-sm">
      <span className="text-slate-400">
        Workout: <span className="text-slate-200">{parsed.workoutId}</span>
      </span>
      <span className="text-slate-400">
        RPE: <span className="text-slate-200">{parsed.rpe}</span>
      </span>
      {parsed.workoutDate ? (
        <span className="text-slate-400">
          Date: <span className="text-slate-200">{parsed.workoutDate}</span>
        </span>
      ) : null}
    </div>
  );
}

function StableConversationSummary({
  conversation,
}: {
  conversation: Record<string, unknown> | null;
}) {
  if (!conversation) {
    return null;
  }

  return (
    <div className="flex flex-wrap gap-4 text-sm">
      <span className="text-slate-400">
        Conversation:{' '}
        <span className="font-mono text-slate-200">{String(conversation.conversationId ?? '')}</span>
      </span>
      <span className="text-slate-400">
        Surface: <span className="text-slate-200">{String(conversation.surface ?? '')}</span>
      </span>
      <span className="text-slate-400">
        Focus: <span className="text-slate-200">{String(conversation.focus ?? '')}</span>
      </span>
    </div>
  );
}

function StableMesoWindow({
  windowStart,
  windowEnd,
}: {
  windowStart: string | null;
  windowEnd: string | null;
}) {
  if (!windowStart && !windowEnd) {
    return null;
  }

  return (
    <div className="flex flex-wrap gap-4 text-sm">
      {windowStart ? (
        <span className="text-slate-400">
          Meso start: <span className="text-slate-200">{windowStart}</span>
        </span>
      ) : null}
      {windowEnd ? (
        <span className="text-slate-400">
          Meso end: <span className="text-slate-200">{windowEnd}</span>
        </span>
      ) : null}
    </div>
  );
}

function StableSavedAt({ savedAtEpochSeconds }: { savedAtEpochSeconds: string | null }) {
  if (!savedAtEpochSeconds) {
    return null;
  }

  return (
    <div className="text-sm text-slate-400">
      Saved at epoch: <span className="font-mono text-slate-200">{savedAtEpochSeconds}</span>
    </div>
  );
}

function StableTextBlock({ text }: { text: string | null }) {
  if (!text) {
    return null;
  }

  return (
    <div className="prompt-preview-text rounded-xl border border-white/10 bg-white/[0.02] px-4 py-3 text-sm text-slate-300">
      {text}
    </div>
  );
}

function StableMarkdownDetails({ title, text }: { title: string; text: string | null }) {
  if (!text) {
    return null;
  }

  return (
    <details className="rounded-xl border border-white/10 bg-white/[0.02]">
      <summary className="cursor-pointer px-4 py-2 text-xs font-semibold uppercase tracking-[0.15em] text-slate-500">
        {title}
      </summary>
      <div className="prompt-preview-text border-t border-white/5 px-4 py-3 text-sm">
        <MarkdownContent>{text}</MarkdownContent>
      </div>
    </details>
  );
}

function StableMesoRoadmap({
  guidance,
  roadmap,
}: {
  guidance: string | null;
  roadmap: Record<string, unknown> | null;
}) {
  if (!guidance && !roadmap) {
    return null;
  }

  return (
    <details className="rounded-xl border border-white/10 bg-white/[0.02]">
      <summary className="cursor-pointer px-4 py-2 text-xs font-semibold uppercase tracking-[0.15em] text-slate-500">
        Meso Cycle Roadmap (predicted)
      </summary>
      <div className="space-y-3 border-t border-white/5 px-4 py-3 text-sm">
        {guidance ? <p className="prompt-preview-text text-slate-300">{guidance}</p> : null}
        {roadmap ? <DecodedPackedContext label="Meso Cycle Roadmap" data={roadmap} /> : null}
      </div>
    </details>
  );
}
