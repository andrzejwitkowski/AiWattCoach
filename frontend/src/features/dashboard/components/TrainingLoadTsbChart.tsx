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

type TrainingLoadTsbChartProps = {
  axisLabels: AxisLabel[];
  timelineLabels: TimelineLabel[];
  ariaLabel: string;
  currentTsb: string;
  latestSnapshotDate: string | null;
  latestSnapshotValue: string | null;
  hoveredSnapshotDate: string | null;
  hoveredSnapshotValue: string | null;
  focusPoint: ChartPoint | null;
  hoveredPoint: ChartPoint | null;
  hoveredTooltipTop: number;
  linePath: string;
  areaPath: string;
  showArea: boolean;
  gradientId: string;
  freshnessHeight: number;
  optimalHeight: number;
  riskHeight: number;
  onHover: (event: MouseEvent<SVGSVGElement>) => void;
  onLeave: () => void;
  strings: {
    formLabel: string;
    freshnessPeakLabel: string;
    optimalTrainingLabel: string;
    highRiskLabel: string;
    latestSnapshotLabel: string;
    snapshotLabel: string;
  };
};

export function TrainingLoadTsbChart({
  axisLabels,
  timelineLabels,
  ariaLabel,
  currentTsb,
  latestSnapshotDate,
  latestSnapshotValue,
  hoveredSnapshotDate,
  hoveredSnapshotValue,
  focusPoint,
  hoveredPoint,
  hoveredTooltipTop,
  linePath,
  areaPath,
  showArea,
  gradientId,
  freshnessHeight,
  optimalHeight,
  riskHeight,
  onHover,
  onLeave,
  strings,
}: TrainingLoadTsbChartProps) {
  return (
    <section className="overflow-hidden rounded-[2rem] border border-white/8 bg-[#10161d] p-7 shadow-[0_30px_80px_rgba(2,8,23,0.45)]">
      <div className="mb-6 flex flex-wrap items-start justify-between gap-6">
        <Metric label={strings.formLabel} value={currentTsb} tone="lime" />
        {latestSnapshotDate && latestSnapshotValue ? (
          <div className="rounded-2xl border border-white/8 bg-white/[0.03] px-4 py-3 text-right">
            <p className="text-[10px] font-black uppercase tracking-[0.24em] text-lime-300/80">{strings.latestSnapshotLabel}</p>
            <p className="mt-2 text-sm font-semibold text-white">{latestSnapshotDate}</p>
            <p className="mt-1 text-xs uppercase tracking-[0.18em] text-slate-500">{latestSnapshotValue}</p>
          </div>
        ) : null}
        <div className="flex flex-wrap gap-4 text-[10px] font-black uppercase tracking-[0.18em] text-slate-500">
          <LegendDot label={strings.freshnessPeakLabel} tone="cyan" />
          <LegendDot label={strings.optimalTrainingLabel} tone="lime" />
          <LegendDot label={strings.highRiskLabel} tone="red" />
        </div>
      </div>
      <div className="relative h-72 overflow-hidden rounded-[1.6rem] border border-white/8 bg-[#0b1117] pl-12 pr-4 pt-4">
        <div className="absolute inset-0 flex flex-col">
          <div className="relative border-b border-cyan-300/10 bg-cyan-300/6" style={{ height: `${freshnessHeight}%` }}>
            <span className="absolute bottom-3 right-4 text-[9px] font-black uppercase tracking-[0.22em] text-cyan-300/45">{strings.freshnessPeakLabel}</span>
          </div>
          <div className="relative border-b border-lime-300/10 bg-lime-300/6" style={{ height: `${optimalHeight}%` }}>
            <span className="absolute bottom-3 right-4 text-[9px] font-black uppercase tracking-[0.22em] text-lime-300/45">{strings.optimalTrainingLabel}</span>
          </div>
          <div className="relative bg-red-400/6" style={{ height: `${riskHeight}%` }}>
            <span className="absolute bottom-3 right-4 text-[9px] font-black uppercase tracking-[0.22em] text-red-300/45">{strings.highRiskLabel}</span>
          </div>
        </div>
        <div className="pointer-events-none absolute inset-0 bg-[linear-gradient(to_right,rgba(249,250,251,0.04)_1px,transparent_1px),linear-gradient(to_bottom,rgba(249,250,251,0.04)_1px,transparent_1px)] bg-[size:40px_40px]" />
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
              <div className="h-full w-px bg-lime-300/30" />
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
            <defs>
              <linearGradient id={gradientId} x1="0" x2="0" y1="0" y2="1">
                <stop offset="0%" stopColor="#bef264" stopOpacity="0.26" />
                <stop offset="100%" stopColor="#bef264" stopOpacity="0" />
              </linearGradient>
            </defs>
            {showArea ? <path d={areaPath} fill={`url(#${gradientId})`} /> : null}
            <path d={linePath} fill="none" stroke="#bef264" strokeWidth="0.5" strokeLinecap="round" />
            {focusPoint ? <circle cx={focusPoint.x} cy={focusPoint.y} r="0.6" fill="#d9f99d" /> : null}
          </svg>
          {hoveredSnapshotDate && hoveredSnapshotValue && hoveredPoint ? (
            <div
              className="pointer-events-none absolute z-20 w-44 -translate-x-1/2 rounded-2xl border border-lime-300/20 bg-[#1a232c]/90 px-4 py-3 shadow-[0_20px_50px_rgba(0,0,0,0.45)] backdrop-blur"
              style={{ left: `${Math.min(Math.max(hoveredPoint.x, 18), 82)}%`, top: `${hoveredTooltipTop}%` }}
            >
              <p className="text-[10px] font-black uppercase tracking-[0.24em] text-lime-300">{strings.snapshotLabel}</p>
              <p className="mt-2 text-sm font-semibold text-white">{hoveredSnapshotDate}</p>
              <p className="mt-2 text-xs uppercase tracking-[0.18em] text-slate-400">{strings.formLabel}</p>
              <p className="mt-1 text-lg font-black text-lime-200">{hoveredSnapshotValue}</p>
            </div>
          ) : null}
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

function LegendDot({ label, tone }: { label: string; tone: 'cyan' | 'lime' | 'red' }) {
  const toneClass = tone === 'cyan' ? 'bg-cyan-300/40' : tone === 'lime' ? 'bg-lime-300/35' : 'bg-red-300/35';

  return (
    <span className="inline-flex items-center gap-2">
      <span className={`h-2.5 w-2.5 rounded-full ${toneClass}`} />
      <span>{label}</span>
    </span>
  );
}

function Metric({ label, value, tone }: { label: string; value: string; tone: 'lime' }) {
  const toneClass = tone === 'lime' ? 'bg-lime-300' : 'bg-lime-300';

  return (
    <div className="flex items-center gap-3">
      <div className={`h-1.5 w-12 rounded-full ${toneClass}`} />
      <div>
        <p className="text-[10px] font-black uppercase tracking-[0.22em] text-slate-500">{label}</p>
        <p className="text-[2rem] font-black leading-none text-lime-200">{value}</p>
      </div>
    </div>
  );
}
