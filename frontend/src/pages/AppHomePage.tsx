import { useEffect, useState } from 'react';

import { loadTrainingLoadDashboard } from '../features/dashboard/api/dashboard';
import { TrainingLoadReport } from '../features/dashboard/components/TrainingLoadReport';
import type { DashboardRange, TrainingLoadDashboardResponse } from '../features/dashboard/types';

type AppHomePageProps = {
  apiBaseUrl: string;
};

export function AppHomePage({ apiBaseUrl }: AppHomePageProps) {
  const [range, setRange] = useState<DashboardRange>('90d');
  const [report, setReport] = useState<TrainingLoadDashboardResponse | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;

    setLoading(true);
    setError(null);

    void loadTrainingLoadDashboard(apiBaseUrl, range)
      .then((payload) => {
        if (!cancelled) {
          setReport(payload);
        }
      })
      .catch((err) => {
        if (!cancelled) {
          setError(err instanceof Error ? err.message : 'Failed to load dashboard');
          setReport(null);
        }
      })
      .finally(() => {
        if (!cancelled) {
          setLoading(false);
        }
      });

    return () => {
      cancelled = true;
    };
  }, [apiBaseUrl, range]);

  if (loading) {
    return (
      <div className="flex min-h-[18rem] items-center justify-center rounded-[2rem] border border-white/10 bg-[#0d141f]">
        <p className="text-sm uppercase tracking-[0.24em] text-slate-400">Loading dashboard</p>
      </div>
    );
  }

  if (error || !report) {
    return (
      <section className="rounded-[2rem] border border-red-500/25 bg-red-500/10 p-8 text-center shadow-[0_20px_60px_rgba(127,29,29,0.25)]">
        <p className="text-sm font-semibold uppercase tracking-[0.24em] text-red-300">Dashboard unavailable</p>
        <p className="mt-4 text-base text-red-100">{error ?? 'Failed to load dashboard.'}</p>
      </section>
    );
  }

  return <TrainingLoadReport report={report} range={range} onRangeChange={setRange} />;
}
