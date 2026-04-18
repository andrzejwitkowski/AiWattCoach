import { useId, useState, type MouseEvent } from 'react';
import { useTranslation } from 'react-i18next';

import type { TrainingLoadDashboardResponse } from '../types';
import {
  buildAreaPath,
  buildLinePath,
  buildPositionedAxisLabels,
  buildSeriesPoints,
  clamp,
  latestSeriesPoint,
  resolveHoveredIndex,
  seriesPointAtIndex,
} from './trainingLoadChartUtils';
import { buildTimelineLabels, formatMetricValue, formatShortDate } from './trainingLoadFormatters';
import { TrainingLoadLoadChart } from './TrainingLoadLoadChart';
import { TrainingLoadTsbChart } from './TrainingLoadTsbChart';

type TrainingLoadChartsProps = {
  report: TrainingLoadDashboardResponse;
};

export function TrainingLoadCharts({ report }: TrainingLoadChartsProps) {
  const { i18n, t } = useTranslation();
  const language = i18n.resolvedLanguage ?? i18n.language ?? 'en';
  const gradientId = useId().replace(/:/g, '');
  const [activeLoadIndex, setActiveLoadIndex] = useState<number | null>(null);
  const [activeTsbIndex, setActiveTsbIndex] = useState<number | null>(null);
  const ctlValues = report.points.map((point) => point.currentCtl);
  const atlValues = report.points.map((point) => point.currentAtl);
  const tsbValues = report.points.map((point) => point.currentTsb);
  const loadNumbers = [...ctlValues, ...atlValues].filter((value): value is number => value !== null);
  const tsbNumbers = tsbValues.filter((value): value is number => value !== null);
  const loadMin = Math.min(...loadNumbers, 0);
  const loadMax = Math.max(...loadNumbers, 100);
  const tsbMin = Math.min(...tsbNumbers, -60);
  const tsbMax = Math.max(...tsbNumbers, 40);
  const loadAxisLabels = buildPositionedAxisLabels([
    loadMax,
    loadMin + ((loadMax - loadMin) * 2) / 3,
    loadMin + (loadMax - loadMin) / 3,
    loadMin,
  ], loadMin, loadMax);
  const timelineLabels = buildTimelineLabels(report.points.map((point) => point.date), language);
  const tsbAxisLabels = buildPositionedAxisLabels([tsbMax, 0, -30, tsbMin], tsbMin, tsbMax);
  const ctlPoints = buildSeriesPoints(ctlValues, loadMin, loadMax, 100, 100);
  const atlPoints = buildSeriesPoints(atlValues, loadMin, loadMax, 100, 100);
  const tsbPoints = buildSeriesPoints(tsbValues, tsbMin, tsbMax, 100, 100);
  const latestLoadPoint = latestSeriesPoint(ctlPoints, atlPoints);
  const latestTsbPoint = latestSeriesPoint(tsbPoints);
  const latestLoadSnapshot = latestLoadPoint ? report.points[latestLoadPoint.index] : null;
  const latestTsbSnapshot = latestTsbPoint ? report.points[latestTsbPoint.index] : null;
  const latestLoadMarkerColor = latestLoadSnapshot && latestLoadSnapshot.currentCtl === null ? '#ff7a45' : '#22d3ee';
  const highlightedLoadSeriesLabel = latestLoadSnapshot && latestLoadSnapshot.currentCtl === null ? 'fatigue' : 'fitness';
  const hoveredLoadPoint = seriesPointAtIndex(ctlPoints, activeLoadIndex) ?? seriesPointAtIndex(atlPoints, activeLoadIndex);
  const hoveredTsbPoint = seriesPointAtIndex(tsbPoints, activeTsbIndex);
  const hoveredLoadSnapshot = activeLoadIndex === null ? null : report.points[activeLoadIndex] ?? null;
  const hoveredTsbSnapshot = activeTsbIndex === null ? null : report.points[activeTsbIndex] ?? null;
  const loadFocusPoint = hoveredLoadPoint ?? latestLoadPoint;
  const tsbFocusPoint = hoveredTsbPoint ?? latestTsbPoint;
  const loadFocusMarkerColor = hoveredLoadSnapshot
    ? (hoveredLoadSnapshot.currentCtl === null ? '#ff7a45' : '#22d3ee')
    : latestLoadMarkerColor;
  const hoveredLoadTooltipTop = hoveredLoadPoint ? clamp(hoveredLoadPoint.y - 20, 7, 60) : 7;
  const hoveredTsbTooltipTop = hoveredTsbPoint ? clamp(hoveredTsbPoint.y - 18, 7, 62) : 7;
  const tsbRange = Math.max(tsbMax - tsbMin, 1);
  const freshnessBoundary = clamp(((tsbMax - clamp(0, tsbMin, tsbMax)) / tsbRange) * 100, 0, 100);
  const riskBoundary = clamp(((tsbMax - clamp(-30, tsbMin, tsbMax)) / tsbRange) * 100, freshnessBoundary, 100);
  const freshnessHeight = freshnessBoundary;
  const optimalHeight = riskBoundary - freshnessBoundary;
  const riskHeight = 100 - riskBoundary;
  const tsbHasGap = tsbValues.some((value) => value === null);
  const ctlLinePath = buildLinePath(ctlValues, loadMin, loadMax, 100, 100);
  const atlLinePath = buildLinePath(atlValues, loadMin, loadMax, 100, 100);
  const tsbLinePath = buildLinePath(tsbValues, tsbMin, tsbMax, 100, 100);
  const tsbAreaPath = buildAreaPath(tsbPoints, 100);
  const latestLoadSnapshotDate = latestLoadSnapshot ? formatShortDate(latestLoadSnapshot.date, language) : null;
  const latestLoadSnapshotMetrics = latestLoadSnapshot
    ? t('dashboard.charts.load.latestSnapshot.summary', {
        ctl: formatMetricValue(latestLoadSnapshot.currentCtl, language),
        atl: formatMetricValue(latestLoadSnapshot.currentAtl, language),
      })
    : null;
  const hoveredLoadSnapshotDate = hoveredLoadSnapshot ? formatShortDate(hoveredLoadSnapshot.date, language) : null;
  const hoveredLoadSnapshotMetrics = hoveredLoadSnapshot
    ? t('dashboard.charts.load.tooltip.summary', {
        ctl: formatMetricValue(hoveredLoadSnapshot.currentCtl, language),
        atl: formatMetricValue(hoveredLoadSnapshot.currentAtl, language),
      })
    : null;
  const latestTsbSnapshotDate = latestTsbSnapshot ? formatShortDate(latestTsbSnapshot.date, language) : null;
  const latestTsbSnapshotValue = latestTsbSnapshot ? formatMetricValue(latestTsbSnapshot.currentTsb, language) : null;
  const hoveredTsbSnapshotDate = hoveredTsbSnapshot ? formatShortDate(hoveredTsbSnapshot.date, language) : null;
  const hoveredTsbSnapshotValue = hoveredTsbSnapshot ? formatMetricValue(hoveredTsbSnapshot.currentTsb, language) : null;
  const timelineStartLabel = timelineLabels[0]?.label ?? t('dashboard.charts.shared.timeline.startFallback');
  const timelineEndLabel = timelineLabels[timelineLabels.length - 1]?.label ?? t('dashboard.charts.shared.timeline.latestFallback');
  const loadLatestSnapshotAria = latestLoadSnapshotDate && latestLoadSnapshot
    ? t('dashboard.charts.load.aria.latestSnapshotAvailable', {
        date: latestLoadSnapshotDate,
        ctl: formatMetricValue(latestLoadSnapshot.currentCtl, language),
        atl: formatMetricValue(latestLoadSnapshot.currentAtl, language),
      })
    : t('dashboard.charts.load.aria.latestSnapshotUnavailable');
  const tsbLatestSnapshotAria = latestTsbSnapshotDate && latestTsbSnapshotValue
    ? t('dashboard.charts.tsb.aria.latestSnapshotAvailable', {
        date: latestTsbSnapshotDate,
        tsb: latestTsbSnapshotValue,
      })
    : t('dashboard.charts.tsb.aria.latestSnapshotUnavailable');
  const loadAriaLabel = t('dashboard.charts.load.aria.description', {
    start: timelineStartLabel,
    end: timelineEndLabel,
    latestSnapshot: loadLatestSnapshotAria,
    series: highlightedLoadSeriesLabel === 'fatigue'
      ? t('dashboard.charts.load.aria.series.fatigue')
      : t('dashboard.charts.load.aria.series.fitness'),
  });
  const tsbAriaLabel = t('dashboard.charts.tsb.aria.description', {
    start: timelineStartLabel,
    end: timelineEndLabel,
    latestSnapshot: tsbLatestSnapshotAria,
  });

  const handleLoadHover = (event: MouseEvent<SVGSVGElement>) => {
    const nextIndex = resolveHoveredIndex(event, report.points.length);
    setActiveLoadIndex((current) => current === nextIndex ? current : nextIndex);
  };

  const handleLoadLeave = () => {
    setActiveLoadIndex(null);
  };

  const handleTsbHover = (event: MouseEvent<SVGSVGElement>) => {
    const nextIndex = resolveHoveredIndex(event, report.points.length);
    setActiveTsbIndex((current) => current === nextIndex ? current : nextIndex);
  };

  const handleTsbLeave = () => {
    setActiveTsbIndex(null);
  };

  return (
    <div className="space-y-6">
      <TrainingLoadLoadChart
        axisLabels={loadAxisLabels}
        timelineLabels={timelineLabels}
        ariaLabel={loadAriaLabel}
        currentCtl={formatMetricValue(report.summary.currentCtl, language)}
        currentAtl={formatMetricValue(report.summary.currentAtl, language)}
        latestSnapshotDate={latestLoadSnapshotDate}
        latestSnapshotMetrics={latestLoadSnapshotMetrics}
        hoveredSnapshotDate={hoveredLoadSnapshotDate}
        hoveredSnapshotMetrics={hoveredLoadSnapshotMetrics}
        focusPoint={loadFocusPoint}
        hoveredPoint={hoveredLoadPoint}
        hoveredTooltipTop={hoveredLoadTooltipTop}
        focusMarkerColor={loadFocusMarkerColor}
        ctlLinePath={ctlLinePath}
        atlLinePath={atlLinePath}
        onHover={handleLoadHover}
        onLeave={handleLoadLeave}
        strings={{
          fitnessLabel: t('dashboard.charts.load.metrics.fitness'),
          fatigueLabel: t('dashboard.charts.load.metrics.fatigue'),
          latestSnapshotLabel: t('dashboard.charts.load.latestSnapshot.eyebrow'),
          snapshotLabel: t('dashboard.charts.load.tooltip.eyebrow'),
        }}
      />

      <TrainingLoadTsbChart
        axisLabels={tsbAxisLabels}
        timelineLabels={timelineLabels}
        ariaLabel={tsbAriaLabel}
        currentTsb={formatMetricValue(report.summary.currentTsb, language)}
        latestSnapshotDate={latestTsbSnapshotDate}
        latestSnapshotValue={latestTsbSnapshotValue}
        hoveredSnapshotDate={hoveredTsbSnapshotDate}
        hoveredSnapshotValue={hoveredTsbSnapshotValue}
        focusPoint={tsbFocusPoint}
        hoveredPoint={hoveredTsbPoint}
        hoveredTooltipTop={hoveredTsbTooltipTop}
        linePath={tsbLinePath}
        areaPath={tsbAreaPath}
        showArea={!tsbHasGap}
        gradientId={gradientId}
        freshnessHeight={freshnessHeight}
        optimalHeight={optimalHeight}
        riskHeight={riskHeight}
        onHover={handleTsbHover}
        onLeave={handleTsbLeave}
        strings={{
          formLabel: t('dashboard.charts.tsb.metric'),
          freshnessPeakLabel: t('dashboard.charts.tsb.legend.freshnessPeak'),
          optimalTrainingLabel: t('dashboard.charts.tsb.legend.optimalTraining'),
          highRiskLabel: t('dashboard.charts.tsb.legend.highRisk'),
          latestSnapshotLabel: t('dashboard.charts.tsb.latestSnapshot.eyebrow'),
          snapshotLabel: t('dashboard.charts.tsb.tooltip.eyebrow'),
        }}
      />
    </div>
  );
}
