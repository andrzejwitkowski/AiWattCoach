import { useMemo } from 'react';
import { decodeShortKey, formatSeconds } from '../utils/decodePackedContext';

type DecodedPackedContextProps = {
  label: string;
  data: Record<string, unknown>;
};

function isObject(v: unknown): v is Record<string, unknown> {
  return typeof v === 'object' && v !== null && !Array.isArray(v);
}

export function DecodedPackedContext({ label, data }: DecodedPackedContextProps) {
  const sections = useMemo(() => buildSections(data), [data]);

  return (
    <div className="space-y-4">
      <div className="text-xs font-semibold uppercase tracking-[0.15em] text-slate-500">{label}</div>
      {sections.map((section, i) => (
        <div key={i} className="rounded-xl border border-white/10 bg-white/[0.02] px-4 py-3">
          {section}
        </div>
      ))}
    </div>
  );
}

function buildSections(data: Record<string, unknown>): React.ReactNode[] {
  const sections: React.ReactNode[] = [];

  if (data.p && isObject(data.p)) {
    sections.push(buildProfileTable(data.p));
  }
  if (data.rc && Array.isArray(data.rc)) {
    sections.push(buildRaceTable(data.rc));
  }
  if (data.h && isObject(data.h)) {
    sections.push(buildHistorySummary(data.h));
  }
  if (data.rd && Array.isArray(data.rd)) {
    sections.push(buildRecentDays(data.rd));
  }

  const rest: Record<string, unknown> = {};
  for (const key of Object.keys(data)) {
    if (!['p', 'rc', 'h', 'rd', 'ud', 'pd', 'fe', 'v', 'g', 'fx', 'i'].includes(key)) {
      rest[key] = data[key];
    }
  }

  const rawSection = buildRawTable(rest);
  if (rawSection) sections.push(rawSection);

  return sections;
}

function buildProfileTable(p: Record<string, unknown>) {
  const rows: { label: string; value: string }[] = [];
  if (p.fnm) rows.push({ label: 'Name', value: String(p.fnm) });
  if (p.age) rows.push({ label: 'Age', value: String(p.age) });
  if (p.hcm) rows.push({ label: 'Height', value: `${p.hcm} cm` });
  if (p.wkg) rows.push({ label: 'Weight', value: `${p.wkg} kg` });
  if (p.ftp) rows.push({ label: 'FTP', value: `${p.ftp} W` });
  if (p.hrm) rows.push({ label: 'Max HR', value: String(p.hrm) });
  if (p.vo2) rows.push({ label: 'VO₂max', value: String(p.vo2) });
  if (p.meds) rows.push({ label: 'Medications', value: String(p.meds) });
  if (p.notes) rows.push({ label: 'Notes', value: String(p.notes) });
  return (
    <div>
      <div className="mb-2 text-xs font-semibold uppercase tracking-[0.15em] text-slate-500">Athlete Profile</div>
      <div className="grid grid-cols-2 gap-x-6 gap-y-1 text-sm md:grid-cols-3">
        {rows.map((r) => (
          <div key={r.label} className="flex justify-between gap-2">
            <span className="text-slate-400">{r.label}</span>
            <span className="text-slate-200">{r.value}</span>
          </div>
        ))}
      </div>
    </div>
  );
}

function buildRaceTable(races: unknown[]) {
  return (
    <div>
      <div className="mb-2 text-xs font-semibold uppercase tracking-[0.15em] text-slate-500">Race Calendar</div>
      <table className="w-full text-sm">
        <thead>
          <tr className="border-b border-white/10 text-left text-xs uppercase text-slate-500">
            <th className="pb-1 pr-3">Date</th>
            <th className="pb-1 pr-3">Name</th>
            <th className="pb-1 pr-3">Dist</th>
            <th className="pb-1 pr-3">Priority</th>
          </tr>
        </thead>
        <tbody>
          {races.map((item, i) => {
            const r = item as Record<string, unknown>;
            return (
              <tr key={i} className="border-b border-white/5 text-slate-200">
                <td className="py-1 pr-3 text-slate-400">{String(r.d ?? '')}</td>
                <td className="py-1 pr-3">{String(r.n ?? '')}</td>
                <td className="py-1 pr-3">{r.km ? `${r.km} km` : ''}</td>
                <td className="py-1 pr-3">
                  <PriorityBadge priority={String(r.pri ?? '')} />
                </td>
              </tr>
            );
          })}
        </tbody>
      </table>
    </div>
  );
}

function PriorityBadge({ priority }: { priority: string }) {
  const colors: Record<string, string> = {
    A: 'bg-red-500/20 text-red-300',
    B: 'bg-amber-500/20 text-amber-300',
    C: 'bg-slate-500/20 text-slate-300',
  };
  return (
    <span className={`rounded px-1.5 py-0.5 text-xs font-semibold ${colors[priority] ?? 'bg-white/10 text-slate-300'}`}>
      {priority}
    </span>
  );
}

function buildHistorySummary(h: Record<string, unknown>) {
  const metrics: { label: string; value: string }[] = [];
  if (h.ac) metrics.push({ label: 'Activities', value: String(h.ac) });
  if (h.ttss) metrics.push({ label: 'Total TSS', value: String(h.ttss) });
  if (h.ctl != null) metrics.push({ label: 'CTL', value: numStr(h.ctl) });
  if (h.atl != null) metrics.push({ label: 'ATL', value: numStr(h.atl) });
  if (h.tsb != null) metrics.push({ label: 'TSB', value: numStr(h.tsb) });
  if (h.ftp) metrics.push({ label: 'FTP', value: `${h.ftp} W` });
  if (h.ftpd) metrics.push({ label: 'FTP Δ', value: numStr(h.ftpd) });
  if (h.t7) metrics.push({ label: 'Avg TSS 7d', value: numStr(h.t7) });
  if (h.t28) metrics.push({ label: 'Avg TSS 28d', value: numStr(h.t28) });

  return (
    <div>
      <div className="mb-2 text-xs font-semibold uppercase tracking-[0.15em] text-slate-500">Training Load Summary</div>
      <div className="mb-3 grid grid-cols-2 gap-x-6 gap-y-1 text-sm md:grid-cols-4">
        {metrics.map((m) => (
          <div key={m.label} className="flex justify-between gap-2">
            <span className="text-slate-400">{m.label}</span>
            <span className="text-slate-200">{m.value}</span>
          </div>
        ))}
      </div>
      {Array.isArray(h.lt) && h.lt.length > 0 && (
        <details>
          <summary className="cursor-pointer text-xs font-semibold uppercase tracking-wider text-slate-500">
            Load Trend ({h.lt.length} days)
          </summary>
          <div className="mt-2 max-h-48 overflow-auto">
            <table className="w-full text-xs">
              <thead>
                <tr className="border-b border-white/10 text-left text-slate-500">
                  <th className="pb-1 pr-2">Date</th>
                  <th className="pb-1 pr-2">TSS</th>
                  <th className="pb-1 pr-2">CTL</th>
                  <th className="pb-1 pr-2">ATL</th>
                  <th className="pb-1 pr-2">TSB</th>
                </tr>
              </thead>
              <tbody>
                {(h.lt as unknown[]).slice(-21).map((pt, i) => {
                  const p = pt as Record<string, unknown>;
                  const tsbVal = Number(p.tsb);
                  return (
                    <tr key={i} className="border-b border-white/5 text-slate-300">
                      <td className="py-0.5 pr-2 text-slate-500">{String(p.d ?? '')}</td>
                      <td className="py-0.5 pr-2">{String(p.tss ?? '')}</td>
                      <td className="py-0.5 pr-2">{numStr(p.ctl)}</td>
                      <td className="py-0.5 pr-2">{numStr(p.atl)}</td>
                      <td className={`py-0.5 pr-2 ${!Number.isNaN(tsbVal) && tsbVal < -5 ? 'text-red-300' : !Number.isNaN(tsbVal) && tsbVal > 5 ? 'text-green-300' : ''}`}>
                        {numStr(p.tsb)}
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>
        </details>
      )}
    </div>
  );
}

function buildRecentDays(rd: unknown[]) {
  return (
    <div>
      <div className="mb-2 text-xs font-semibold uppercase tracking-[0.15em] text-slate-500">Recent Days</div>
      <div className="space-y-2">
        {(rd as unknown[]).slice(-7).map((item, i) => {
          const d = item as Record<string, unknown>;
          return (
            <details key={i}>
              <summary className="cursor-pointer rounded-lg border border-white/10 bg-white/[0.03] px-3 py-2 text-sm">
                <span className="text-slate-300">{String(d.d ?? '')}</span>
                {d.fr ? <span className="ml-2 text-xs text-cyan-400">Free day</span> : null}
                {d.sick ? <span className="ml-2 text-xs text-red-400">Sick</span> : null}
                {Array.isArray(d.w) && d.w.length > 0 ? (
                  <span className="ml-2 text-xs text-slate-500">
                    {d.w.length} workout{d.w.length > 1 ? 's' : ''}
                  </span>
                ) : (
                  <span className="ml-2 text-xs text-slate-500">Rest</span>
                )}
              </summary>
              {Array.isArray(d.w) && d.w.length > 0 && (
                <div className="mt-2 space-y-2 pl-2">
                  {(d.w as unknown[]).map((w, j) => {
                    const workout = w as Record<string, unknown>;
                    return (
                      <div key={j} className="rounded-lg border border-white/5 bg-white/[0.02] px-3 py-2">
                        <div className="flex flex-wrap items-center gap-x-3 gap-y-0.5 text-sm">
                          <span className="font-medium text-slate-200">{String(workout.n ?? '')}</span>
                          <span className="text-xs text-slate-500">{String(workout.ty ?? '')}</span>
                          {workout.dur ? <span className="text-xs text-slate-400">{formatSeconds(Number(workout.dur))}</span> : null}
                          {workout.tss != null ? <span className="text-xs text-slate-400">{String(workout.tss)} TSS</span> : null}
                          {workout.ifv != null ? <span className="text-xs text-slate-400">IF {numStr(workout.ifv)}</span> : null}
                          {workout.rpe != null ? <RPEDisplay rpe={Number(workout.rpe)} /> : null}
                        </div>
                        {typeof workout.recap === 'string' && workout.recap.length > 80 && (
                          <details className="mt-1">
                            <summary className="cursor-pointer text-xs text-slate-500">Recap snippet</summary>
                            <div className="prompt-preview-text mt-1 text-xs leading-5 text-slate-400">
                              {(workout.recap as string).slice(0, 300)}
                              {(workout.recap as string).length > 300 ? '…' : ''}
                            </div>
                          </details>
                        )}
                      </div>
                    );
                  })}
                </div>
              )}
            </details>
          );
        })}
      </div>
    </div>
  );
}

function RPEDisplay({ rpe }: { rpe: number }) {
  const colors: Record<number, string> = {
    1: 'text-green-400',
    2: 'text-green-300',
    3: 'text-sky-300',
    4: 'text-sky-200',
    5: 'text-amber-300',
    6: 'text-amber-200',
    7: 'text-orange-300',
    8: 'text-red-300',
    9: 'text-red-400',
    10: 'text-red-500',
  };
  return <span className={`text-xs font-semibold ${colors[rpe] ?? 'text-slate-400'}`}>RPE {rpe}</span>;
}

function buildRawTable(data: Record<string, unknown>) {
  const entries = Object.entries(data).filter(([, v]) => {
    if (v == null) return false;
    if (Array.isArray(v) && v.length === 0) return false;
    if (isObject(v) && Object.keys(v).length === 0) return false;
    return true;
  });
  if (entries.length === 0) return null;
  return (
    <div>
      <div className="mb-2 text-xs font-semibold uppercase tracking-[0.15em] text-slate-500">Other Fields</div>
      <div className="grid grid-cols-2 gap-x-4 gap-y-1 text-sm md:grid-cols-3">
        {entries.map(([key, val]) => (
          <div key={key} className="flex justify-between gap-2">
            <span className="text-slate-400">{decodeShortKey(key)}</span>
            <span className="text-slate-200">{fmtVal(val)}</span>
          </div>
        ))}
      </div>
    </div>
  );
}

function fmtVal(v: unknown): string {
  if (typeof v === 'number') return String(v);
  if (typeof v === 'string') return v;
  if (Array.isArray(v)) return `[${v.length} items]`;
  if (isObject(v)) return `{${Object.keys(v).length} fields}`;
  return String(v);
}

function numStr(v: unknown): string {
  if (typeof v === 'number') return v.toFixed(1);
  return String(v ?? '');
}
