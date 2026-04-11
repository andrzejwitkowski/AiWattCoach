type ChartBar = {
  height: number;
  color: string;
  widthUnits?: number;
};

type CalendarMiniChartProps = {
  bars: Array<number | ChartBar>;
  tone: 'primary' | 'secondary' | 'error' | 'anaerobic' | 'muted' | 'race';
};

const TONE_CLASS: Record<CalendarMiniChartProps['tone'], string> = {
  primary: 'bg-[#d2ff9a]',
  secondary: 'bg-[#00e3fd]',
  error: 'bg-[#ff7351]',
  anaerobic: 'bg-[#800020]',
  muted: 'bg-slate-700',
  race: 'bg-[#d49c45]',
};

export function CalendarMiniChart({ bars, tone }: CalendarMiniChartProps) {
  if (bars.length === 0) {
    return null;
  }

  return (
    <div className="mb-2 flex h-10 items-end gap-[1px]">
      {bars.map((bar, index) => {
        const normalizedBar = typeof bar === 'number' && Number.isFinite(bar) ? bar : typeof bar === 'object' && Number.isFinite(bar.height) ? bar.height : 20;
        const height = Math.max(20, Math.min(100, normalizedBar));
        const inlineColor = typeof bar === 'object' ? bar.color : undefined;
        const widthUnits = typeof bar === 'object' && Number.isFinite(bar.widthUnits) ? Math.max(1, bar.widthUnits ?? 1) : 1;

        return (
          <div
            key={`${tone}-${index}-${typeof bar === 'number' ? bar : `${bar.height}-${bar.color}`}`}
            data-chart-bar="mini"
            className={`min-w-0 rounded-t-[1px] ${TONE_CLASS[tone]}`}
            style={{ flexBasis: 0, flexGrow: widthUnits, height: `${height}%`, ...(inlineColor ? { backgroundColor: inlineColor } : {}) }}
          />
        );
      })}
    </div>
  );
}
