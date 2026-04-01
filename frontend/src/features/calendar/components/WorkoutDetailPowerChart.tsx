import {type MouseEvent, useState} from 'react';

export type ChartIntervalOverlay = {
  id: string;
  startSecond: number;
  endSecond: number;
  label: string;
};

type ChartSamplePoint = {
  value: number;
  second: number;
};

type PowerChartProps = {
  activeInterval: ChartIntervalOverlay | null;
  activeIntervalKey: string | null;
  intervals: ChartIntervalOverlay[];
  onHoverIntervalChange: (intervalKey: string | null) => void;
  onSelectIntervalChange: (intervalKey: string | null) => void;
  sampleDurationSeconds?: number;
  title: string;
  values: number[];
};

export function PowerChart({
  activeInterval,
  activeIntervalKey,
  intervals,
  onHoverIntervalChange,
  onSelectIntervalChange,
  sampleDurationSeconds = 1,
  title,
  values,
}: PowerChartProps) {
  const totalSeconds = Math.max(
    values.length * sampleDurationSeconds,
    intervals.reduce((max, interval) => Math.max(max, interval.endSecond), 0),
    1,
  );
  const sampledPoints = samplePowerValues(values, 180, sampleDurationSeconds);
  const [hoveredSampleIndex, setHoveredSampleIndex] = useState<number | null>(null);

  if (sampledPoints.length === 0) {
    return null;
  }

  const hoveredSample = hoveredSampleIndex !== null ? sampledPoints[hoveredSampleIndex] : null;
  const pinnedSample = activeInterval ? samplePointForInterval(sampledPoints, activeInterval) : null;
  const displayedSample = hoveredSample ?? pinnedSample;
  const maxValue = Math.max(...sampledPoints.map((point) => point.value), 1);
  const chartHeight = 220;
  const chartWidth = 1000;
  const points = sampledPoints
    .map((point, index) => {
      const x = sampledPoints.length === 1 ? 0 : (index / (sampledPoints.length - 1)) * chartWidth;
      const normalized = Math.max(0, point.value) / maxValue;
      const y = chartHeight - (normalized * chartHeight);
      return `${x},${y}`;
    })
    .join(' ');

  const markerIndex = hoveredSampleIndex ?? (pinnedSample
    ? sampledPoints.findIndex((point) => point.second === pinnedSample.second && point.value === pinnedSample.value)
    : null);
  const markerX = markerIndex === null || markerIndex < 0 || sampledPoints.length === 1
    ? null
    : (markerIndex / (sampledPoints.length - 1)) * chartWidth;
  const markerY = displayedSample === null
    ? null
    : chartHeight - ((Math.max(0, displayedSample.value) / maxValue) * chartHeight);

  const handleChartPointerMove = (event: MouseEvent<SVGSVGElement>) => {
    const bounds = event.currentTarget.getBoundingClientRect();
    if (bounds.width <= 0 || sampledPoints.length === 0) {
      return;
    }

    const relativeX = Math.max(0, Math.min(1, (event.clientX - bounds.left) / bounds.width));
    const nextIndex = Math.round(relativeX * Math.max(0, sampledPoints.length - 1));
    setHoveredSampleIndex(nextIndex);

    const hoveredSecond = sampledPoints[nextIndex]?.second ?? 0;
    const nextInterval = intervals.find((interval) => hoveredSecond >= interval.startSecond && hoveredSecond <= interval.endSecond);
    onHoverIntervalChange(nextInterval?.id ?? null);
  };

  return (
    <div className="rounded-2xl border border-white/6 bg-[#171a1d] p-4">
      <div className="flex items-start justify-between gap-4">
        <p className="text-[10px] font-black uppercase tracking-[0.24em] text-slate-500">{title}</p>
        <div className="flex items-center gap-4">
          {displayedSample ? (
            <p
              data-hover-power-readout="true"
              className="text-xs font-bold uppercase tracking-[0.18em] text-slate-300"
            >
              {formatChartTimeLabel(displayedSample.second)} • {displayedSample.value} W
            </p>
          ) : null}
          <p className="text-xs font-bold uppercase tracking-[0.18em] text-[#d2ff9a]">{maxValue} W max (5s avg)</p>
        </div>
      </div>
      <div className="mt-4 overflow-hidden rounded-2xl border border-white/5 bg-[linear-gradient(180deg,rgba(210,255,154,0.16)_0%,rgba(210,255,154,0.03)_100%)] p-3">
        <svg
          aria-label={title}
          className="h-56 w-full"
          data-power-chart="true"
          viewBox={`0 0 ${chartWidth} ${chartHeight}`}
          onMouseLeave={() => {
            setHoveredSampleIndex(null);
            onHoverIntervalChange(null);
          }}
          onMouseMove={handleChartPointerMove}
          preserveAspectRatio="none"
          role="img"
        >
          <defs>
            <linearGradient id="power-chart-stroke" x1="0%" y1="0%" x2="100%" y2="0%">
              <stop offset="0%" stopColor="#52c41a" />
              <stop offset="55%" stopColor="#d2ff9a" />
              <stop offset="100%" stopColor="#facc15" />
            </linearGradient>
          </defs>
          {intervals.map((interval, index) => {
            const startX = (Math.max(0, interval.startSecond) / totalSeconds) * chartWidth;
            const endX = (Math.max(interval.startSecond, interval.endSecond) / totalSeconds) * chartWidth;
            const width = Math.max(6, endX - startX);
            const isActive = interval.id === activeIntervalKey;

            return (
              <g key={`${interval.label}-${index}-${interval.startSecond}`}>
                <rect
                  data-interval-overlay="true"
                  x={startX}
                  y={0}
                  width={width}
                  height={chartHeight}
                  fill={isActive ? 'rgba(210,255,154,0.16)' : index % 2 === 0 ? 'rgba(255,255,255,0.05)' : 'rgba(255,255,255,0.02)'}
                />
                <line
                  x1={startX}
                  x2={startX}
                  y1={0}
                  y2={chartHeight}
                  stroke={isActive ? 'rgba(210,255,154,0.32)' : 'rgba(255,255,255,0.08)'}
                  strokeWidth="2"
                />
              </g>
            );
          })}
          <path d={`M 0 ${chartHeight} L ${points} L ${chartWidth} ${chartHeight} Z`} fill="rgba(210,255,154,0.18)" />
          <polyline
            fill="none"
            points={points}
            stroke="url(#power-chart-stroke)"
            strokeLinecap="round"
            strokeLinejoin="round"
            strokeWidth="4"
          />
          {displayedSample !== null && markerX !== null && markerY !== null ? (
            <g data-power-chart-marker="true">
              <line
                x1={markerX}
                x2={markerX}
                y1={0}
                y2={chartHeight}
                stroke="rgba(255,255,255,0.22)"
                strokeDasharray="8 8"
                strokeWidth="2"
              />
              <circle cx={markerX} cy={markerY} r="6" fill="#d2ff9a" stroke="#111417" strokeWidth="3" />
            </g>
          ) : null}
        </svg>
      </div>
      <div className="mt-3 flex items-center justify-between gap-2 text-[10px] font-bold uppercase tracking-[0.16em] text-slate-500">
        {buildTimeTicks(totalSeconds).map((tick, index) => (
          <span key={`${index}-${tick.second}-${tick.label}`}>{tick.label}</span>
        ))}
      </div>
      {intervals.length ? (
        <div className="mt-3 flex flex-wrap gap-2">
          {intervals.map((interval, index) => (
            <span
              key={`${interval.label}-${interval.startSecond}-${index}`}
              data-interval-chip-active={interval.id === activeIntervalKey ? 'true' : 'false'}
              className={`rounded-full border px-3 py-1 text-[10px] font-bold uppercase tracking-[0.16em] transition ${interval.id === activeIntervalKey ? 'border-[#d2ff9a]/40 bg-[#d2ff9a]/12 text-[#f4ffd9]' : 'border-white/8 bg-white/[0.04] text-slate-300'}`}
              onClick={() => onSelectIntervalChange(activeIntervalKey === interval.id ? null : interval.id)}
              onMouseEnter={() => onHoverIntervalChange(interval.id)}
              onMouseLeave={() => onHoverIntervalChange(null)}
              onKeyDown={(event) => {
                if (event.key === 'Enter' || event.key === ' ') {
                  event.preventDefault();
                  onSelectIntervalChange(activeIntervalKey === interval.id ? null : interval.id);
                }
              }}
              role="button"
              tabIndex={0}
            >
              {interval.label}
            </span>
          ))}
        </div>
      ) : null}
    </div>
  );
}

function samplePowerValues(values: number[], maxPoints: number, sampleDurationSeconds: number): ChartSamplePoint[] {
  if (values.length <= maxPoints) {
    return values.map((value, index) => ({value, second: index * sampleDurationSeconds}));
  }

  const bucketSize = values.length / maxPoints;
  const sampled: ChartSamplePoint[] = [];

  for (let index = 0; index < maxPoints; index += 1) {
    const start = Math.floor(index * bucketSize);
    const end = Math.min(values.length, Math.floor((index + 1) * bucketSize));
    const bucket = values.slice(start, Math.max(start + 1, end));
    const average = bucket.reduce((sum, value) => sum + value, 0) / bucket.length;
    sampled.push({
      value: Math.round(average),
      second: Math.round(((start + Math.max(start, end - 1)) / 2) * sampleDurationSeconds),
    });
  }

  return sampled;
}

function samplePointForInterval(points: ChartSamplePoint[], interval: ChartIntervalOverlay): ChartSamplePoint | null {
  if (points.length === 0) {
    return null;
  }

  const midpoint = interval.startSecond + ((interval.endSecond - interval.startSecond) / 2);

  return points.reduce((closest, point) => {
    if (closest === null) {
      return point;
    }

    const closestDistance = Math.abs(closest.second - midpoint);
    const nextDistance = Math.abs(point.second - midpoint);
    return nextDistance < closestDistance ? point : closest;
  }, null as ChartSamplePoint | null);
}

function buildTimeTicks(totalSeconds: number): Array<{ second: number; label: string }> {
  const safeTotalSeconds = Math.max(1, totalSeconds - 1);
  return [0, 0.25, 0.5, 0.75, 1].map((ratio) => {
    const second = Math.round(safeTotalSeconds * ratio);
    return {
      second,
      label: formatChartTimeLabel(second),
    };
  });
}

function formatChartTimeLabel(totalSeconds: number): string {
  const safeSeconds = Math.max(0, Math.round(totalSeconds));
  const hours = Math.floor(safeSeconds / 3600);
  const minutes = Math.floor((safeSeconds % 3600) / 60);
  const seconds = safeSeconds % 60;

  if (hours > 0) {
    return `${hours}:${String(minutes).padStart(2, '0')}:${String(seconds).padStart(2, '0')}`;
  }

  return `${minutes}:${String(seconds).padStart(2, '0')}`;
}
