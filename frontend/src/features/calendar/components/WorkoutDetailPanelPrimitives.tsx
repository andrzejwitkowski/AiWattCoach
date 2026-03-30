import type {WorkoutBar} from '../workoutDetails';

export function WorkoutBars({bars}: { bars: WorkoutBar[] }) {
  if (bars.length === 0) {
    return <div className="h-40 rounded-2xl border border-white/6 bg-[#171a1d]" />;
  }

  return (
    <div className="flex h-48 items-end gap-1 rounded-2xl border border-white/6 bg-[#171a1d] p-4">
      {bars.map((bar, index) => (
        <div
          key={`${bar.color}-${index}-${bar.height}-${bar.widthUnits ?? 1}`}
          data-chart-bar="detail"
          className="min-w-[6px] rounded-t-md"
          style={{
            flexBasis: 0,
            flexGrow: Math.max(1, bar.widthUnits ?? 1),
            height: `${bar.height}%`,
            backgroundColor: bar.color,
          }}
        />
      ))}
    </div>
  );
}

export function MetricCard({label, value}: { label: string; value: string }) {
  return (
    <div className="rounded-2xl border border-white/6 bg-[#171a1d] p-4">
      <p className="text-[10px] font-black uppercase tracking-[0.24em] text-slate-500">{label}</p>
      <p className="mt-2 text-xl font-black text-[#f9f9fd]">{value}</p>
    </div>
  );
}
