import {resolvePowerZoneColor} from '../workoutDetails';

type ChartBar = {
  height: number;
  color: string;
  widthUnits?: number;
};

export type CalendarPowerTrace = {
  kind: 'power-trace';
  values: number[];
  ftpWatts: number | null;
};

export type CalendarLegacyBars = {
  kind: 'bars';
  bars: Array<number | ChartBar>;
};

export type CalendarMiniChartData = CalendarLegacyBars | CalendarPowerTrace;

type CalendarMiniChartProps = {
  chart: CalendarMiniChartData;
  tone: 'primary' | 'secondary' | 'error' | 'anaerobic' | 'muted' | 'race';
};

const TONE_COLOR: Record<CalendarMiniChartProps['tone'], string> = {
  primary: '#d2ff9a',
  secondary: '#00e3fd',
  error: '#ff7351',
  anaerobic: '#800020',
  muted: '#334155',
  race: '#d49c45',
};

function compressMiniChartWidth(widthUnits: number | undefined): number {
  if (!widthUnits || !Number.isFinite(widthUnits) || widthUnits <= 0) {
    return 1;
  }

  // Keep interval order and broad time proportion, but compress long durations
  // so short intervals stay visible in the small calendar chart.
  return Math.max(1, Math.round(Math.sqrt(widthUnits)));
}

export function CalendarMiniChart({ chart, tone }: CalendarMiniChartProps) {
  if (chart.kind === 'power-trace') {
    return <CalendarPowerTraceChart trace={chart} />;
  }

  const {bars} = chart;
  if (bars.length === 0) {
    return null;
  }

  return (
    <div className="mb-2 flex h-16 items-end gap-[1px]">
      {bars.map((bar, index) => {
        const normalizedBar = typeof bar === 'number' && Number.isFinite(bar) ? bar : typeof bar === 'object' && Number.isFinite(bar.height) ? bar.height : 20;
        const height = Math.max(4, Math.min(100, normalizedBar));
        const inlineColor = typeof bar === 'object' ? bar.color : undefined;
        const widthUnits = typeof bar === 'object' ? compressMiniChartWidth(bar.widthUnits) : 1;

        return (
          <div
            key={`${tone}-${index}-${typeof bar === 'number' ? bar : `${bar.height}-${bar.color}`}`}
            data-chart-bar="mini"
            className="min-w-[2px] rounded-t-[1px]"
            style={{ flexBasis: 0, flexGrow: widthUnits, height: `${height}%`, backgroundColor: inlineColor ?? TONE_COLOR[tone] }}
          />
        );
      })}
    </div>
  );
}

function CalendarPowerTraceChart({trace}: { trace: CalendarPowerTrace }) {
  const sampled = sampleTraceValues(trace.values, 36);
  if (sampled.length === 0) {
    return null;
  }

  const width = 100;
  const height = 58;
  const maxValue = Math.max(...sampled, 1);
  const points = sampled.map((value, index) => {
    const x = sampled.length === 1 ? 0 : (index / (sampled.length - 1)) * width;
    const y = height - ((Math.max(0, value) / maxValue) * (height - 4));
    return `${x},${y}`;
  }).join(' ');

  return (
    <svg
      aria-label="calendar power trace"
      className="mb-2 h-16 w-full overflow-hidden rounded-lg border border-white/5 bg-[#11171b]"
      data-calendar-power-trace="true"
      preserveAspectRatio="none"
      viewBox={`0 0 ${width} ${height}`}
    >
      <g data-calendar-power-zone-area="true" opacity="0.68">
        {sampled.map((value, index) => {
          const x = (index / sampled.length) * width;
          const nextX = ((index + 1) / sampled.length) * width;
          const y = height - ((Math.max(0, value) / maxValue) * (height - 4));
          return (
            <rect
              key={`${index}-${value}`}
              x={x}
              y={y}
              width={Math.max(1, nextX - x)}
              height={height - y}
              fill={resolvePowerZoneColor(value, trace.ftpWatts)}
            />
          );
        })}
      </g>
      <polyline
        fill="none"
        points={points}
        stroke="#eaffbf"
        strokeLinecap="round"
        strokeLinejoin="round"
        strokeWidth="3"
      />
      <polyline
        fill="none"
        points={points}
        stroke="#caff62"
        strokeLinecap="round"
        strokeLinejoin="round"
        strokeWidth="1.5"
      />
    </svg>
  );
}

function sampleTraceValues(values: number[], maxPoints: number): number[] {
  const finiteValues = values.filter(Number.isFinite);
  if (finiteValues.length <= maxPoints) {
    return finiteValues.map((value) => Math.round(value));
  }

  const bucketSize = finiteValues.length / maxPoints;
  return Array.from({length: maxPoints}, (_, index) => {
    const start = Math.floor(index * bucketSize);
    const end = Math.min(finiteValues.length, Math.max(start + 1, Math.floor((index + 1) * bucketSize)));
    const bucket = finiteValues.slice(start, end);
    return Math.round(bucket.reduce((sum, value) => sum + value, 0) / bucket.length);
  });
}
