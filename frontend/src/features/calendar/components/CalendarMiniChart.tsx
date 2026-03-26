type CalendarMiniChartProps = {
  bars: number[];
  tone: 'primary' | 'secondary' | 'error' | 'anaerobic' | 'muted';
};

const TONE_CLASS: Record<CalendarMiniChartProps['tone'], string> = {
  primary: 'bg-[#d2ff9a]',
  secondary: 'bg-[#00e3fd]',
  error: 'bg-[#ff7351]',
  anaerobic: 'bg-[#800020]',
  muted: 'bg-slate-700',
};

export function CalendarMiniChart({ bars, tone }: CalendarMiniChartProps) {
  if (bars.length === 0) {
    return null;
  }

  return (
    <div className="mb-2 flex h-10 items-end gap-[1px]">
      {bars.map((bar, index) => (
        <div
          key={`${tone}-${index}-${bar}`}
          className={`flex-1 rounded-t-[1px] ${TONE_CLASS[tone]}`}
          style={{ height: `${Math.max(20, Math.min(100, bar))}%` }}
        />
      ))}
    </div>
  );
}
