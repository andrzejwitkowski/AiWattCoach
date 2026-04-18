import { useId, useState, type MouseEvent } from 'react';

import type { TrainingLoadDashboardResponse } from '../types';

type TrainingLoadChartsProps = {
  report: TrainingLoadDashboardResponse;
};

type SeriesPoint = {
  index: number;
  value: number;
  x: number;
  y: number;
};

const CHART_X_INSET = 2;

function clamp(value: number, min: number, max: number) {
  return Math.min(Math.max(value, min), max);
}

function buildPointX(index: number, total: number, width: number) {
  return total === 1 ? width / 2 : CHART_X_INSET + (index / (total - 1)) * (width - (CHART_X_INSET * 2));
}

function buildSeriesPoints(values: Array<number | null>, min: number, max: number, height: number, width: number): SeriesPoint[] {
  const denominator = Math.max(max - min, 1);

  return values.flatMap((value, index) => {
    if (value === null) {
      return [];
    }

    const x = buildPointX(index, values.length, width);
    const y = height - ((value - min) / denominator) * height;
    return [{ index, value, x, y }];
  });
}

function buildLinePath(values: Array<number | null>, min: number, max: number, height: number, width: number) {
  const denominator = Math.max(max - min, 1);
  let segmentOpen = false;

  return values.reduce((path, value, index) => {
    if (value === null) {
      segmentOpen = false;
      return path;
    }

    const x = buildPointX(index, values.length, width);
    const y = height - ((value - min) / denominator) * height;
    const command = `${segmentOpen ? 'L' : 'M'}${x.toFixed(2)},${y.toFixed(2)}`;
    segmentOpen = true;
    return path ? `${path} ${command}` : command;
  }, '');
}

function buildAreaPath(points: SeriesPoint[], height: number) {
  if (points.length < 2) {
    return '';
  }

  const line = points.map((point, index) => `${index === 0 ? 'M' : 'L'}${point.x.toFixed(2)},${point.y.toFixed(2)}`).join(' ');
  return `${line} L${points[points.length - 1].x.toFixed(2)},${height.toFixed(2)} L${points[0].x.toFixed(2)},${height.toFixed(2)} Z`;
}

function formatAxisLabel(value: number) {
  return Number.isInteger(value) ? value.toString() : value.toFixed(0);
}

function formatMetricValue(value: number | null) {
  return value === null ? '-' : value.toFixed(1);
}

function formatShortDate(date: string) {
  const parsed = new Date(`${date}T00:00:00Z`);
  return Number.isNaN(parsed.getTime())
    ? date
    : new Intl.DateTimeFormat('en-US', { month: 'short', day: '2-digit', timeZone: 'UTC' }).format(parsed);
}

function buildPositionedAxisLabels(values: number[], min: number, max: number) {
  const range = Math.max(max - min, 1);
  const seen = new Set<string>();

  return values.flatMap((value) => {
    if (value < min || value > max) {
      return [];
    }

    const key = value.toFixed(3);
    if (seen.has(key)) {
      return [];
    }
    seen.add(key);

    return [{
      key,
      label: formatAxisLabel(value),
      top: clamp(((max - value) / range) * 100, 0, 100),
    }];
  });
}

function buildTimelineLabels(dates: string[]) {
  if (dates.length === 0) {
    return [] as Array<{ key: string; label: string }>;
  }

  const candidateIndexes = Array.from(new Set([
    0,
    Math.floor((dates.length - 1) * 0.33),
    Math.floor((dates.length - 1) * 0.66),
    dates.length - 1,
  ]));

  return candidateIndexes.map((index) => ({ key: `${dates[index]}-${index}`, label: formatShortDate(dates[index]) }));
}

function latestSeriesPoint(...seriesCollections: SeriesPoint[][]) {
  return seriesCollections.flat().reduce<SeriesPoint | null>((latest, point) => {
    if (!latest || point.index > latest.index) {
      return point;
    }

    return latest;
  }, null);
}

function seriesPointAtIndex(points: SeriesPoint[], index: number | null) {
  if (index === null) {
    return null;
  }

  return points.find((point) => point.index === index) ?? null;
}

function resolveHoveredIndex(event: MouseEvent<SVGSVGElement>, totalPoints: number) {
  if (totalPoints === 0) {
    return null;
  }

  if (totalPoints === 1) {
    return 0;
  }

  const bounds = event.currentTarget.getBoundingClientRect();
  if (bounds.width <= 0) {
    return totalPoints - 1;
  }

  const ratio = clamp((event.clientX - bounds.left) / bounds.width, 0, 1);
  return Math.round(ratio * (totalPoints - 1));
}

export function TrainingLoadCharts({ report }: TrainingLoadChartsProps) {
  const gradientId = useId().replace(/:/g, '');
  const [activeLoadIndex, setActiveLoadIndex] = useState<number | null>(null);
  const [activeTsbIndex, setActiveTsbIndex] = useState<number | null>(null);
  const ctlValues = report.points.map((point) => point.currentCtl);
  const atlValues = report.points.map((point) => point.currentAtl);
  const tsbValues = report.points.map((point) => point.currentTsb);
  const loadNumbers = [...ctlValues, ...atlValues].filter((value): value is number => value !== null);
  const tsbNumbers = tsbValues.filter((value): value is number => value !== null);
  const loadMin = Math.min(...loadNumbers, 0);
  const loadMax = Math.max(...loadNumbers, 100);
  const tsbMin = Math.min(...tsbNumbers, -60);
  const tsbMax = Math.max(...tsbNumbers, 40);
  const loadAxisLabels = buildPositionedAxisLabels([
    loadMax,
    loadMin + ((loadMax - loadMin) * 2) / 3,
    loadMin + (loadMax - loadMin) / 3,
    loadMin,
  ], loadMin, loadMax);
  const timelineLabels = buildTimelineLabels(report.points.map((point) => point.date));
  const tsbAxisLabels = buildPositionedAxisLabels([tsbMax, 0, -30, tsbMin], tsbMin, tsbMax);
  const ctlPoints = buildSeriesPoints(ctlValues, loadMin, loadMax, 100, 100);
  const atlPoints = buildSeriesPoints(atlValues, loadMin, loadMax, 100, 100);
  const tsbPoints = buildSeriesPoints(tsbValues, tsbMin, tsbMax, 100, 100);
  const latestLoadPoint = latestSeriesPoint(ctlPoints, atlPoints);
  const latestTsbPoint = latestSeriesPoint(tsbPoints);
  const latestLoadSnapshot = latestLoadPoint ? report.points[latestLoadPoint.index] : null;
  const latestTsbSnapshot = latestTsbPoint ? report.points[latestTsbPoint.index] : null;
  const latestLoadMarkerColor = latestLoadSnapshot && latestLoadSnapshot.currentCtl === null ? '#ff7a45' : '#22d3ee';
  const hoveredLoadPoint = seriesPointAtIndex(ctlPoints, activeLoadIndex) ?? seriesPointAtIndex(atlPoints, activeLoadIndex);
  const hoveredTsbPoint = seriesPointAtIndex(tsbPoints, activeTsbIndex);
  const hoveredLoadSnapshot = activeLoadIndex === null ? null : report.points[activeLoadIndex] ?? null;
  const hoveredTsbSnapshot = activeTsbIndex === null ? null : report.points[activeTsbIndex] ?? null;
  const loadFocusPoint = hoveredLoadPoint ?? latestLoadPoint;
  const tsbFocusPoint = hoveredTsbPoint ?? latestTsbPoint;
  const loadFocusMarkerColor = hoveredLoadSnapshot
    ? (hoveredLoadSnapshot.currentCtl === null ? '#ff7a45' : '#22d3ee')
    : latestLoadMarkerColor;
  const hoveredLoadTooltipTop = hoveredLoadPoint ? clamp(hoveredLoadPoint.y - 20, 7, 60) : 7;
  const hoveredTsbTooltipTop = hoveredTsbPoint ? clamp(hoveredTsbPoint.y - 18, 7, 62) : 7;
  const tsbRange = Math.max(tsbMax - tsbMin, 1);
  const freshnessBoundary = clamp(((tsbMax - clamp(0, tsbMin, tsbMax)) / tsbRange) * 100, 0, 100);
  const riskBoundary = clamp(((tsbMax - clamp(-30, tsbMin, tsbMax)) / tsbRange) * 100, freshnessBoundary, 100);
  const freshnessHeight = freshnessBoundary;
  const optimalHeight = riskBoundary - freshnessBoundary;
  const riskHeight = 100 - riskBoundary;
  const tsbHasGap = tsbValues.some((value) => value === null);

  const handleLoadHover = (event: MouseEvent<SVGSVGElement>) => {
    const nextIndex = resolveHoveredIndex(event, report.points.length);
    setActiveLoadIndex((current) => current === nextIndex ? current : nextIndex);
  };

  const handleTsbHover = (event: MouseEvent<SVGSVGElement>) => {
    const nextIndex = resolveHoveredIndex(event, report.points.length);
    setActiveTsbIndex((current) => current === nextIndex ? current : nextIndex);
  };

  return (
    <div className="space-y-6">
      <section className="overflow-hidden rounded-[2rem] border border-white/8 bg-[#10161d] p-7 shadow-[0_30px_80px_rgba(2,8,23,0.45)]">
        <div className="mb-6 flex flex-wrap items-start justify-between gap-6">
          <div className="flex flex-wrap gap-8">
            <Metric label="Fitness (CTL)" value={report.summary.currentCtl} tone="cyan" />
            <Metric label="Fatigue (ATL)" value={report.summary.currentAtl} tone="orange" />
          </div>
          {latestLoadSnapshot ? (
            <div className="rounded-2xl border border-white/8 bg-white/[0.03] px-4 py-3 text-right">
              <p className="text-[10px] font-black uppercase tracking-[0.24em] text-lime-300/80">Latest snapshot</p>
              <p className="mt-2 text-sm font-semibold text-white">{formatShortDate(latestLoadSnapshot.date)}</p>
              <p className="mt-1 text-xs uppercase tracking-[0.18em] text-slate-500">
                CTL {formatMetricValue(latestLoadSnapshot.currentCtl)} / ATL {formatMetricValue(latestLoadSnapshot.currentAtl)}
              </p>
            </div>
          ) : null}
        </div>
        <div className="relative h-72 overflow-hidden rounded-[1.6rem] border border-white/8 bg-[linear-gradient(to_right,rgba(249,250,251,0.045)_1px,transparent_1px),linear-gradient(to_bottom,rgba(249,250,251,0.045)_1px,transparent_1px)] bg-[size:40px_40px] pl-12 pr-4 pt-4">
          <div className="pointer-events-none absolute inset-y-4 left-4 z-10 w-7 text-[10px] font-bold uppercase tracking-[0.18em] text-slate-500">
            {loadAxisLabels.map((label) => (
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
            {loadFocusPoint ? (
              <div className="pointer-events-none absolute inset-y-0 z-10 -translate-x-px" style={{ left: `${loadFocusPoint.x}%` }}>
                <div className="h-full w-px bg-lime-300/25" />
              </div>
            ) : null}
            {hoveredLoadSnapshot && hoveredLoadPoint ? (
              <div
                className="pointer-events-none absolute z-20 w-48 -translate-x-1/2 rounded-2xl border border-white/12 bg-[#18212a]/92 px-4 py-3 shadow-[0_20px_50px_rgba(0,0,0,0.45)] backdrop-blur"
                style={{ left: `${clamp(hoveredLoadPoint.x, 20, 80)}%`, top: `${hoveredLoadTooltipTop}%` }}
              >
                <p className="text-[10px] font-black uppercase tracking-[0.24em] text-cyan-200/80">Snapshot</p>
                <p className="mt-2 text-sm font-semibold text-white">{formatShortDate(hoveredLoadSnapshot.date)}</p>
                <p className="mt-2 text-xs uppercase tracking-[0.18em] text-slate-400">
                  CTL {formatMetricValue(hoveredLoadSnapshot.currentCtl)} / ATL {formatMetricValue(hoveredLoadSnapshot.currentAtl)}
                </p>
              </div>
            ) : null}
            <svg
              viewBox="0 0 100 100"
              preserveAspectRatio="none"
              className="relative h-full w-full"
              role="img"
              aria-label={`Fitness and fatigue chart from ${timelineLabels[0]?.label ?? 'start'} to ${timelineLabels[timelineLabels.length - 1]?.label ?? 'latest'}. Latest load snapshot ${latestLoadSnapshot ? `${formatShortDate(latestLoadSnapshot.date)} with CTL ${formatMetricValue(latestLoadSnapshot.currentCtl)} and ATL ${formatMetricValue(latestLoadSnapshot.currentAtl)}` : 'is unavailable'}. The highlighted dot follows ${latestLoadSnapshot && latestLoadSnapshot.currentCtl === null ? 'fatigue' : 'fitness'} because that is the latest available load series point.`}
              onMouseEnter={handleLoadHover}
              onMouseMove={handleLoadHover}
              onMouseLeave={() => setActiveLoadIndex(null)}
            >
              <path d={buildLinePath(atlValues, loadMin, loadMax, 100, 100)} fill="none" stroke="#ff7a45" strokeWidth="0.5" strokeLinecap="round" />
              <path d={buildLinePath(ctlValues, loadMin, loadMax, 100, 100)} fill="none" stroke="#22d3ee" strokeWidth="0.5" strokeLinecap="round" />
              {loadFocusPoint ? <circle cx={loadFocusPoint.x} cy={loadFocusPoint.y} r="0.6" fill={loadFocusMarkerColor} /> : null}
            </svg>
          </div>
        </div>
        <div className="mt-4 flex justify-between gap-3 px-1 text-[10px] font-bold uppercase tracking-[0.18em] text-slate-500">
          {timelineLabels.map((label, index) => (
            <span key={label.key} className={index === timelineLabels.length - 1 ? 'text-lime-300' : ''}>{label.label}</span>
          ))}
        </div>
      </section>

      <section className="overflow-hidden rounded-[2rem] border border-white/8 bg-[#10161d] p-7 shadow-[0_30px_80px_rgba(2,8,23,0.45)]">
        <div className="mb-6 flex flex-wrap items-start justify-between gap-6">
          <Metric label="Form (TSB)" value={report.summary.currentTsb} tone="lime" />
          <div className="flex flex-wrap gap-4 text-[10px] font-black uppercase tracking-[0.18em] text-slate-500">
            <LegendDot label="Freshness / Peak" tone="cyan" />
            <LegendDot label="Optimal Training" tone="lime" />
            <LegendDot label="High Risk" tone="red" />
          </div>
        </div>
        <div className="relative h-72 overflow-hidden rounded-[1.6rem] border border-white/8 bg-[#0b1117] pl-12 pr-4 pt-4">
          <div className="absolute inset-0 flex flex-col">
            <div className="relative border-b border-cyan-300/10 bg-cyan-300/6" style={{ height: `${freshnessHeight}%` }}>
              <span className="absolute bottom-3 right-4 text-[9px] font-black uppercase tracking-[0.22em] text-cyan-300/45">Freshness / Peak</span>
            </div>
            <div className="relative border-b border-lime-300/10 bg-lime-300/6" style={{ height: `${optimalHeight}%` }}>
              <span className="absolute bottom-3 right-4 text-[9px] font-black uppercase tracking-[0.22em] text-lime-300/45">Optimal Training</span>
            </div>
            <div className="relative bg-red-400/6" style={{ height: `${riskHeight}%` }}>
              <span className="absolute bottom-3 right-4 text-[9px] font-black uppercase tracking-[0.22em] text-red-300/45">High Risk</span>
            </div>
          </div>
          <div className="pointer-events-none absolute inset-0 bg-[linear-gradient(to_right,rgba(249,250,251,0.04)_1px,transparent_1px),linear-gradient(to_bottom,rgba(249,250,251,0.04)_1px,transparent_1px)] bg-[size:40px_40px]" />
          <div className="pointer-events-none absolute inset-y-4 left-4 z-10 w-7 text-[10px] font-bold uppercase tracking-[0.18em] text-slate-500">
            {tsbAxisLabels.map((label) => (
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
            {tsbFocusPoint ? (
              <div className="pointer-events-none absolute inset-y-0 z-10 -translate-x-px" style={{ left: `${tsbFocusPoint.x}%` }}>
                <div className="h-full w-px bg-lime-300/30" />
              </div>
            ) : null}
            <svg
              viewBox="0 0 100 100"
              preserveAspectRatio="none"
              className="relative h-full w-full"
              role="img"
              aria-label={`Form chart with freshness, optimal training, and high risk zones from ${timelineLabels[0]?.label ?? 'start'} to ${timelineLabels[timelineLabels.length - 1]?.label ?? 'latest'}. Latest TSB snapshot ${latestTsbSnapshot ? `${formatShortDate(latestTsbSnapshot.date)} with TSB ${formatMetricValue(latestTsbSnapshot.currentTsb)}` : 'is unavailable'}.`}
              onMouseEnter={handleTsbHover}
              onMouseMove={handleTsbHover}
              onMouseLeave={() => setActiveTsbIndex(null)}
            >
              <defs>
                <linearGradient id={gradientId} x1="0" x2="0" y1="0" y2="1">
                  <stop offset="0%" stopColor="#bef264" stopOpacity="0.26" />
                  <stop offset="100%" stopColor="#bef264" stopOpacity="0" />
                </linearGradient>
              </defs>
              {!tsbHasGap ? <path d={buildAreaPath(tsbPoints, 100)} fill={`url(#${gradientId})`} /> : null}
              <path d={buildLinePath(tsbValues, tsbMin, tsbMax, 100, 100)} fill="none" stroke="#bef264" strokeWidth="0.5" strokeLinecap="round" />
              {tsbFocusPoint ? <circle cx={tsbFocusPoint.x} cy={tsbFocusPoint.y} r="0.6" fill="#d9f99d" /> : null}
            </svg>
            {hoveredTsbSnapshot && hoveredTsbPoint ? (
              <div
                className="pointer-events-none absolute z-20 w-44 -translate-x-1/2 rounded-2xl border border-lime-300/20 bg-[#1a232c]/90 px-4 py-3 shadow-[0_20px_50px_rgba(0,0,0,0.45)] backdrop-blur"
                style={{ left: `${clamp(hoveredTsbPoint.x, 18, 82)}%`, top: `${hoveredTsbTooltipTop}%` }}
              >
                <p className="text-[10px] font-black uppercase tracking-[0.24em] text-lime-300">Latest snapshot</p>
                <p className="mt-2 text-sm font-semibold text-white">{formatShortDate(hoveredTsbSnapshot.date)}</p>
                <p className="mt-2 text-xs uppercase tracking-[0.18em] text-slate-400">Form (TSB)</p>
                <p className="mt-1 text-lg font-black text-lime-200">{formatMetricValue(hoveredTsbSnapshot.currentTsb)}</p>
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
    </div>
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

function Metric({ label, value, tone }: { label: string; value: number | null; tone: 'cyan' | 'orange' | 'lime' }) {
  const toneClass = tone === 'cyan' ? 'bg-cyan-300' : tone === 'orange' ? 'bg-orange-400' : 'bg-lime-300';
  const valueClass = tone === 'lime' ? 'text-lime-200' : 'text-white';

  return (
    <div className="flex items-center gap-3">
      <div className={`h-1.5 w-12 rounded-full ${toneClass}`} />
      <div>
        <p className="text-[10px] font-black uppercase tracking-[0.22em] text-slate-500">{label}</p>
        <p className={`text-[2rem] font-black leading-none ${valueClass}`}>{value ?? '-'}</p>
      </div>
    </div>
  );
}
