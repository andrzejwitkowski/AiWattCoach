import type { TrainingLoadDashboardResponse } from '../types';

type TrainingLoadInsightCardProps = {
  report: TrainingLoadDashboardResponse;
};

function formatWindowDate(date: string) {
  const parsed = new Date(`${date}T00:00:00Z`);
  return Number.isNaN(parsed.getTime())
    ? date
    : new Intl.DateTimeFormat('en-US', { month: 'short', day: '2-digit', year: 'numeric', timeZone: 'UTC' }).format(parsed);
}

export function TrainingLoadInsightCard({ report }: TrainingLoadInsightCardProps) {
  const delta = report.summary.loadDeltaCtl14d;
  const zone = report.summary.tsbZone;
  const headline = zone === 'freshness_peak'
    ? 'Trending towards peak'
    : zone === 'high_risk'
      ? 'Fatigue is accumulating'
      : 'Productive load block';
  const detail = delta === null
    ? 'Not enough history yet to compare the current load trend with the last two weeks.'
    : `CTL changed by ${delta > 0 ? '+' : ''}${delta.toFixed(1)} over the last 14 days while TSB sits at ${report.summary.currentTsb ?? '-'}. Keep translating this load into adaptation without reintroducing unnecessary fatigue.`;

  return (
    <div className="space-y-6">
      <section className="rounded-[2rem] border border-white/8 bg-[#10161d] p-7 shadow-[0_30px_80px_rgba(2,8,23,0.4)]">
        <div className="flex items-center gap-4">
          <div className="flex h-11 w-11 items-center justify-center rounded-2xl bg-lime-300/10 text-lime-200" aria-hidden="true">
            <span className="text-lg font-black">i</span>
          </div>
          <div>
            <p className="text-[10px] font-black uppercase tracking-[0.24em] text-slate-500">Form Breakdown</p>
            <h3 className="mt-1 text-2xl font-black tracking-tight text-white">Understanding Form (TSB)</h3>
          </div>
        </div>

        <p className="mt-5 text-sm leading-7 text-slate-300">
          Training Stress Balance is the relationship between <span className="font-bold text-cyan-300">Fitness (CTL)</span> and <span className="font-bold text-orange-300">Fatigue (ATL)</span>.
          It helps frame whether you are carrying useful training load, approaching peak freshness, or drifting into excessive risk.
        </p>

        <div className="mt-7 grid gap-4 md:grid-cols-3">
          <ZoneCard title="Freshness / Peak" tone="cyan" detail="Positive TSB. Fatigue has dropped while fitness is still present, so readiness is improving." />
          <ZoneCard title="Optimal Training" tone="lime" detail="TSB sits in the productive band where adaptation is possible without excessive cost." />
          <ZoneCard title="High Risk" tone="red" detail="TSB is too negative. Load may be outpacing recovery, which raises overreaching risk." />
        </div>
      </section>

      <section className="rounded-[2rem] border border-lime-300/12 bg-[radial-gradient(circle_at_top,rgba(190,242,100,0.14),transparent_45%),#0f151b] p-7 shadow-[0_24px_70px_rgba(8,47,73,0.32)]">
        <p className="text-[11px] font-black uppercase tracking-[0.24em] text-lime-300">Coach Insight</p>
        <h3 className="mt-3 text-3xl font-black tracking-tight text-white">{headline}</h3>
        <p className="mt-4 text-sm leading-7 text-slate-300">{detail}</p>
        <dl className="mt-6 grid grid-cols-2 gap-4 text-sm">
          <Metric label="FTP" value={report.summary.ftpWatts !== null ? `${report.summary.ftpWatts} W` : '-'} />
          <Metric label="IF 28d" value={report.summary.averageIf28d !== null ? report.summary.averageIf28d.toFixed(2) : '-'} />
          <Metric label="EF 28d" value={report.summary.averageEf28d !== null ? report.summary.averageEf28d.toFixed(2) : '-'} />
          <Metric label="Window" value={`${formatWindowDate(report.windowStart)} to ${formatWindowDate(report.windowEnd)}`} />
        </dl>
      </section>
    </div>
  );
}

function ZoneCard({ title, detail, tone }: { title: string; detail: string; tone: 'cyan' | 'lime' | 'red' }) {
  const toneClass = tone === 'cyan' ? 'bg-cyan-300' : tone === 'lime' ? 'bg-lime-300' : 'bg-red-300';

  return (
    <div className="space-y-3 rounded-[1.4rem] border border-white/8 bg-white/[0.03] p-4">
      <div className={`h-1.5 w-full rounded-full ${toneClass}`} />
      <p className="text-sm font-black text-white">{title}</p>
      <p className="text-xs leading-6 text-slate-400">{detail}</p>
    </div>
  );
}

function Metric({ label, value }: { label: string; value: string }) {
  return (
    <div className="rounded-2xl border border-white/8 bg-white/[0.04] p-4">
      <dt className="text-[10px] font-black uppercase tracking-[0.22em] text-slate-500">{label}</dt>
      <dd className="mt-2 text-base font-semibold text-white">{value}</dd>
    </div>
  );
}
