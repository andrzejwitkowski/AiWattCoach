import { useTranslation } from 'react-i18next';

import type { DashboardRange, TrainingLoadDashboardResponse } from '../types';
import { TrainingLoadCharts } from './TrainingLoadCharts';
import { TrainingLoadEmptyState } from './TrainingLoadEmptyState';
import { TrainingLoadInsightCard } from './TrainingLoadInsightCard';
import { TrainingLoadRangeSwitch } from './TrainingLoadRangeSwitch';

type TrainingLoadReportProps = {
  report: TrainingLoadDashboardResponse;
  range: DashboardRange;
  onRangeChange: (range: DashboardRange) => void;
};

export function TrainingLoadReport({ report, range, onRangeChange }: TrainingLoadReportProps) {
  const { t } = useTranslation();

  return (
    <section className="space-y-8">
      <div className="flex flex-col gap-5 lg:flex-row lg:items-end lg:justify-between">
        <div className="space-y-3">
          <p className="text-[11px] font-black uppercase tracking-[0.28em] text-lime-300/80">{t('dashboard.report.eyebrow')}</p>
          <div>
            <h2 className="text-4xl font-black tracking-[-0.04em] text-white sm:text-5xl">{t('dashboard.report.title')}</h2>
            <p className="mt-3 text-xs font-semibold uppercase tracking-[0.22em] text-slate-500">
              {t('dashboard.report.subtitle')}
            </p>
          </div>
          <p className="max-w-2xl text-sm leading-7 text-slate-400">
            {t('dashboard.report.description')}
          </p>
        </div>
        <TrainingLoadRangeSwitch value={range} onChange={onRangeChange} />
      </div>

      {!report.hasTrainingLoad ? (
        <TrainingLoadEmptyState />
      ) : (
        <div className="grid gap-6 xl:grid-cols-[minmax(0,1.55fr)_minmax(19rem,0.95fr)]">
          <TrainingLoadCharts report={report} />
          <TrainingLoadInsightCard report={report} />
        </div>
      )}
    </section>
  );
}
