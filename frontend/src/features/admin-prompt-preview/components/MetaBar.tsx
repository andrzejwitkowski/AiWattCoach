import type { AdminPromptPreviewResponse } from '../types';

type MetaBarProps = {
  meta: AdminPromptPreviewResponse['meta'];
};

export function MetaBar({ meta }: MetaBarProps) {
  return (
    <div className="sticky top-0 z-10 flex min-w-0 max-w-full flex-wrap items-center gap-x-4 gap-y-1 rounded-2xl border border-white/10 bg-[#0f1620]/95 px-5 py-3 text-xs uppercase tracking-wider text-slate-400 backdrop-blur">
      <span className="font-semibold text-[#f2c98e]">{meta.surface}</span>
      {meta.selectedWorkoutId ? <span>· {meta.selectedWorkoutId}</span> : null}
      <span className="text-slate-500">·</span>
      <span>{meta.date}</span>
      <span className="text-slate-500">·</span>
      <span className="text-cyan-300">{meta.provider}</span>
      <span className="text-cyan-300/60">{meta.model}</span>
      {meta.focusDate ? (
        <>
          <span className="text-slate-500">·</span>
          <span>focus {meta.focusDate}</span>
        </>
      ) : null}
      {meta.complianceScore != null ? (
        <>
          <span className="text-slate-500">·</span>
          <span className={meta.complianceScore < 0.7 ? 'text-amber-400' : 'text-slate-400'}>
            compliance {Math.round(meta.complianceScore * 100)}%
          </span>
        </>
      ) : null}
      {meta.mesoStart && meta.mesoEnd ? (
        <>
          <span className="text-slate-500">·</span>
          <span>
            meso {meta.mesoStart} → {meta.mesoEnd}
          </span>
        </>
      ) : null}
      {meta.aiCoachLastDate ? (
        <>
          <span className="text-slate-500">·</span>
          <span>ai coach end {meta.aiCoachLastDate}</span>
        </>
      ) : null}
    </div>
  );
}
