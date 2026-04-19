import { Trans, useTranslation } from 'react-i18next';

import type { TrainingLoadDashboardResponse } from '../types';
import { formatMetricValue, formatSignedMetricValue, formatTwoDecimalValue, formatWattsValue, formatWindowDate } from './trainingLoadFormatters';

type TrainingLoadInsightCardProps = {
  report: TrainingLoadDashboardResponse;
};

export function TrainingLoadInsightCard({ report }: TrainingLoadInsightCardProps) {
  const { i18n, t } = useTranslation();
  const language = i18n.resolvedLanguage ?? i18n.language ?? 'en';
  const delta = report.summary.loadDeltaCtl14d;
  const zone = report.summary.tsbZone;
  const headline = zone === 'freshness_peak'
    ? t('dashboard.insights.coachInsight.headlines.freshnessPeak')
    : zone === 'high_risk'
      ? t('dashboard.insights.coachInsight.headlines.highRisk')
      : t('dashboard.insights.coachInsight.headlines.optimalTraining');
  const detail = delta === null
    ? t('dashboard.insights.coachInsight.details.insufficientHistory')
    : t('dashboard.insights.coachInsight.details.trend', {
        delta: formatSignedMetricValue(delta, language),
        tsb: formatMetricValue(report.summary.currentTsb, language),
      });

  return (
    <div className="space-y-6">
      <section className="rounded-[2rem] border border-white/8 bg-[#10161d] p-7 shadow-[0_30px_80px_rgba(2,8,23,0.4)]">
        <div className="flex items-center gap-4">
          <div className="flex h-11 w-11 items-center justify-center rounded-2xl bg-lime-300/10 text-lime-200" aria-hidden="true">
            <span className="text-lg font-black">i</span>
          </div>
          <div>
            <p className="text-[10px] font-black uppercase tracking-[0.24em] text-slate-500">{t('dashboard.insights.formBreakdown.eyebrow')}</p>
            <h3 className="mt-1 text-2xl font-black tracking-tight text-white">{t('dashboard.insights.formBreakdown.title')}</h3>
          </div>
        </div>

        <p className="mt-5 text-sm leading-7 text-slate-300">
          <Trans
            i18nKey="dashboard.insights.formBreakdown.description"
            components={{
              fitness: <span className="font-bold text-cyan-300" />,
              fatigue: <span className="font-bold text-orange-300" />,
            }}
          />
        </p>

        <div className="mt-7 grid gap-4 md:grid-cols-3">
          <ZoneCard
            title={t('dashboard.insights.formBreakdown.zones.freshnessPeak.title')}
            tone="cyan"
            detail={t('dashboard.insights.formBreakdown.zones.freshnessPeak.detail')}
          />
          <ZoneCard
            title={t('dashboard.insights.formBreakdown.zones.optimalTraining.title')}
            tone="lime"
            detail={t('dashboard.insights.formBreakdown.zones.optimalTraining.detail')}
          />
          <ZoneCard
            title={t('dashboard.insights.formBreakdown.zones.highRisk.title')}
            tone="red"
            detail={t('dashboard.insights.formBreakdown.zones.highRisk.detail')}
          />
        </div>
      </section>

      <section className="rounded-[2rem] border border-lime-300/12 bg-[radial-gradient(circle_at_top,rgba(190,242,100,0.14),transparent_45%),#0f151b] p-7 shadow-[0_24px_70px_rgba(8,47,73,0.32)]">
        <p className="text-[11px] font-black uppercase tracking-[0.24em] text-lime-300">{t('dashboard.insights.coachInsight.eyebrow')}</p>
        <h3 className="mt-3 text-3xl font-black tracking-tight text-white">{headline}</h3>
        <p className="mt-4 text-sm leading-7 text-slate-300">{detail}</p>
        <dl className="mt-6 grid grid-cols-2 gap-4 text-sm">
          <Metric label={t('dashboard.insights.coachInsight.metrics.ftp')} value={formatWattsValue(report.summary.ftpWatts, language)} />
          <Metric label={t('dashboard.insights.coachInsight.metrics.if28d')} value={formatTwoDecimalValue(report.summary.averageIf28d, language)} />
          <Metric label={t('dashboard.insights.coachInsight.metrics.ef28d')} value={formatTwoDecimalValue(report.summary.averageEf28d, language)} />
          <Metric
            label={t('dashboard.insights.coachInsight.metrics.window')}
            value={t('dashboard.insights.coachInsight.metrics.windowRange', {
              start: formatWindowDate(report.windowStart, language),
              end: formatWindowDate(report.windowEnd, language),
            })}
          />
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
