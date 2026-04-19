import type { MouseEvent } from 'react';

type AxisLabel = {
  key: string;
  label: string;
  top: number;
};

type TimelineLabel = {
  key: string;
  label: string;
};

type ChartPoint = {
  x: number;
  y: number;
};

type TrainingLoadLoadChartProps = {
  axisLabels: AxisLabel[];
  timelineLabels: TimelineLabel[];
  ariaLabel: string;
  currentCtl: string;
  currentAtl: string;
  latestSnapshotDate: string | null;
  latestSnapshotMetrics: string | null;
  hoveredSnapshotDate: string | null;
  hoveredSnapshotMetrics: string | null;
  focusPoint: ChartPoint | null;
  hoveredPoint: ChartPoint | null;
  hoveredTooltipTop: number;
  focusMarkerColor: string;
  ctlLinePath: string;
  atlLinePath: string;
  onHover: (event: MouseEvent<SVGSVGElement>) => void;
  onLeave: () => void;
  strings: {
    fitnessLabel: string;
    fatigueLabel: string;
    latestSnapshotLabel: string;
    snapshotLabel: string;
  };
};

export function TrainingLoadLoadChart({
  axisLabels,
  timelineLabels,
  ariaLabel,
  currentCtl,
  currentAtl,
  latestSnapshotDate,
  latestSnapshotMetrics,
  hoveredSnapshotDate,
  hoveredSnapshotMetrics,
  focusPoint,
  hoveredPoint,
  hoveredTooltipTop,
  focusMarkerColor,
  ctlLinePath,
  atlLinePath,
  onHover,
  onLeave,
  strings,
}: TrainingLoadLoadChartProps) {
  return (
    <section className="overflow-hidden rounded-[2rem] border border-white/8 bg-[#10161d] p-7 shadow-[0_30px_80px_rgba(2,8,23,0.45)]">
      <div className="mb-6 flex flex-wrap items-start justify-between gap-6">
        <div className="flex flex-wrap gap-8">
          <Metric label={strings.fitnessLabel} value={currentCtl} tone="cyan" />
          <Metric label={strings.fatigueLabel} value={currentAtl} tone="orange" />
        </div>
        {latestSnapshotDate && latestSnapshotMetrics ? (
          <div className="rounded-2xl border border-white/8 bg-white/[0.03] px-4 py-3 text-right">
            <p className="text-[10px] font-black uppercase tracking-[0.24em] text-lime-300/80">{strings.latestSnapshotLabel}</p>
            <p className="mt-2 text-sm font-semibold text-white">{latestSnapshotDate}</p>
            <p className="mt-1 text-xs uppercase tracking-[0.18em] text-slate-500">{latestSnapshotMetrics}</p>
          </div>
        ) : null}
      </div>
      <div className="relative h-72 overflow-hidden rounded-[1.6rem] border border-white/8 bg-[linear-gradient(to_right,rgba(249,250,251,0.045)_1px,transparent_1px),linear-gradient(to_bottom,rgba(249,250,251,0.045)_1px,transparent_1px)] bg-[size:40px_40px] pl-12 pr-4 pt-4">
        <div className="pointer-events-none absolute inset-y-4 left-4 z-10 w-7 text-[10px] font-bold uppercase tracking-[0.18em] text-slate-500">
          {axisLabels.map((label) => (
            <span
              key={label.key}
              className="absolute left-0 -translate-y-1/2"
              style={{ top: `${label.top}%` }}
            >
              {label.label}
            </span>
          ))}
        </div>
        <div className="relative h-full w-full">
          {focusPoint ? (
            <div className="pointer-events-none absolute inset-y-0 z-10 -translate-x-px" style={{ left: `${focusPoint.x}%` }}>
              <div className="h-full w-px bg-lime-300/25" />
            </div>
          ) : null}
          {hoveredSnapshotDate && hoveredSnapshotMetrics && hoveredPoint ? (
            <div
              className="pointer-events-none absolute z-20 w-48 -translate-x-1/2 rounded-2xl border border-white/12 bg-[#18212a]/92 px-4 py-3 shadow-[0_20px_50px_rgba(0,0,0,0.45)] backdrop-blur"
              style={{ left: `${Math.min(Math.max(hoveredPoint.x, 20), 80)}%`, top: `${hoveredTooltipTop}%` }}
            >
              <p className="text-[10px] font-black uppercase tracking-[0.24em] text-cyan-200/80">{strings.snapshotLabel}</p>
              <p className="mt-2 text-sm font-semibold text-white">{hoveredSnapshotDate}</p>
              <p className="mt-2 text-xs uppercase tracking-[0.18em] text-slate-400">{hoveredSnapshotMetrics}</p>
            </div>
          ) : null}
          <svg
            viewBox="0 0 100 100"
            preserveAspectRatio="none"
            className="relative h-full w-full"
            role="img"
            aria-label={ariaLabel}
            onMouseEnter={onHover}
            onMouseMove={onHover}
            onMouseLeave={onLeave}
          >
            <path d={atlLinePath} fill="none" stroke="#ff7a45" strokeWidth="0.5" strokeLinecap="round" />
            <path d={ctlLinePath} fill="none" stroke="#22d3ee" strokeWidth="0.5" strokeLinecap="round" />
            {focusPoint ? <circle cx={focusPoint.x} cy={focusPoint.y} r="0.6" fill={focusMarkerColor} /> : null}
          </svg>
        </div>
      </div>
      <div className="mt-4 flex justify-between gap-3 px-1 text-[10px] font-bold uppercase tracking-[0.18em] text-slate-500">
        {timelineLabels.map((label, index) => (
          <span key={label.key} className={index === timelineLabels.length - 1 ? 'text-lime-300' : ''}>{label.label}</span>
        ))}
      </div>
    </section>
  );
}

function Metric({ label, value, tone }: { label: string; value: string; tone: 'cyan' | 'orange' }) {
  const toneClass = tone === 'cyan' ? 'bg-cyan-300' : 'bg-orange-400';

  return (
    <div className="flex items-center gap-3">
      <div className={`h-1.5 w-12 rounded-full ${toneClass}`} />
      <div>
        <p className="text-[10px] font-black uppercase tracking-[0.22em] text-slate-500">{label}</p>
        <p className="text-[2rem] font-black leading-none text-white">{value}</p>
      </div>
    </div>
  );
}
