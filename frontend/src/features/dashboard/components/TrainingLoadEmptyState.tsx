export function TrainingLoadEmptyState() {
  return (
    <section className="rounded-[2rem] border border-dashed border-white/10 bg-[#0d141f] p-10 text-center shadow-[0_30px_80px_rgba(2,8,23,0.45)]">
      <p className="text-[11px] font-semibold uppercase tracking-[0.24em] text-slate-500">Dashboard</p>
      <h2 className="mt-4 text-3xl font-semibold tracking-tight text-white">Training load will appear here</h2>
      <p className="mx-auto mt-4 max-w-2xl text-sm leading-7 text-slate-400">
        Once completed workouts and daily snapshots are available, this dashboard will show CTL, ATL,
        TSB and the trend of your recent load.
      </p>
    </section>
  );
}
