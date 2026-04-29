type ChartBar = {
  height: number;
  color: string;
  widthUnits?: number;
};

type CalendarMiniChartProps = {
  bars: Array<number | ChartBar>;
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

export function CalendarMiniChart({ bars, tone }: CalendarMiniChartProps) {
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
