import {useRef, useState} from 'react';
import {useTranslation} from 'react-i18next';

import type {IntervalActivity, IntervalEvent} from '../../intervals/types';
import {
  buildCompletedWorkoutPreviewBars,
  buildFiveSecondAveragePowerSeries,
  buildMatchedWorkoutBars,
  extractCompletedPowerValues,
  formatDurationLabel,
} from '../workoutDetails';
import {
  buildChartIntervals,
  CompletedIntervalsSection,
  completedIntervalsTotalDuration,
  firstPositiveValue,
  getDisplayableCompletedIntervals,
  matchedIntervalsTotalDuration,
  MatchedIntervalsSection,
} from './WorkoutDetailIntervalSections';
import {MetricCard, WorkoutBars} from './WorkoutDetailPanelPrimitives';
import {PowerChart} from './WorkoutDetailPowerChart';

export function CompletedWorkoutDetailModal({event, activity}: {
  event: IntervalEvent | null;
  activity: IntervalActivity | null;
}) {
  const {t} = useTranslation();
  const actualWorkout = event?.actualWorkout ?? null;
  const isCompletedActivityOnly = Boolean(!event && activity);
  const isPlannedVsActual = Boolean(event && actualWorkout);
  const detailsUnavailableMessage = !actualWorkout ? activity?.detailsUnavailableReason : null;
  const powerSeries = actualWorkout?.powerValues.length
    ? buildFiveSecondAveragePowerSeries(actualWorkout.powerValues)
    : activity
      ? buildFiveSecondAveragePowerSeries(extractCompletedPowerValues(activity))
      : [];

  const bars = isCompletedActivityOnly && activity
    ? buildCompletedWorkoutPreviewBars(activity)
    : isPlannedVsActual
      ? buildMatchedWorkoutBars(actualWorkout)
      : [];
  const compliance = actualWorkout ? `${Math.round(actualWorkout.complianceScore * 100)}% ${t('calendar.compliance')}` : null;
  const completedIntervals = !actualWorkout ? getDisplayableCompletedIntervals(activity) : [];
  const actualWorkoutDurationSeconds = actualWorkout?.matchedIntervals.reduce((maxDuration, interval) => {
    const intervalEnd = typeof interval.actualEndTimeSeconds === 'number' ? interval.actualEndTimeSeconds : 0;
    return Math.max(maxDuration, intervalEnd);
  }, 0) ?? 0;
  const durationSeconds = isCompletedActivityOnly
    ? firstPositiveValue(activity?.movingTimeSeconds, activity?.elapsedTimeSeconds)
    : isPlannedVsActual
      ? firstPositiveValue(
        activity?.movingTimeSeconds,
        activity?.elapsedTimeSeconds,
        actualWorkoutDurationSeconds || undefined,
      )
      : 0;
  const completedIntervalTotalDurationSeconds = completedIntervalsTotalDuration(completedIntervals, durationSeconds);
  const matchedIntervalTotalDurationSeconds = matchedIntervalsTotalDuration(actualWorkout?.matchedIntervals ?? [], durationSeconds);
  const chartIntervalOverlays = buildChartIntervals(event, actualWorkout, activity);
  const intervalRowRefs = useRef(new Map<string, HTMLButtonElement>());
  const [hoveredIntervalKey, setHoveredIntervalKey] = useState<string | null>(null);
  const [selectedIntervalKey, setSelectedIntervalKey] = useState<string | null>(null);
  const highlightedIntervalKey = chartIntervalOverlays.some((interval) => interval.id === (hoveredIntervalKey ?? selectedIntervalKey))
    ? (hoveredIntervalKey ?? selectedIntervalKey)
    : null;
  const activeInterval = chartIntervalOverlays.find((interval) => interval.id === highlightedIntervalKey) ?? null;

  const normalizedPowerLabel = isCompletedActivityOnly
    ? activity?.metrics.normalizedPowerWatts !== null && activity?.metrics.normalizedPowerWatts !== undefined
      ? `${activity.metrics.normalizedPowerWatts} W`
      : '--'
    : actualWorkout?.normalizedPowerWatts !== null && actualWorkout?.normalizedPowerWatts !== undefined
      ? `${actualWorkout.normalizedPowerWatts} W`
      : '--';
  const trainingStressLabel = isCompletedActivityOnly
    ? activity?.metrics.trainingStressScore !== null && activity?.metrics.trainingStressScore !== undefined
      ? `${activity.metrics.trainingStressScore} TSS`
      : '--'
    : actualWorkout?.trainingStressScore !== null && actualWorkout?.trainingStressScore !== undefined
      ? `${actualWorkout.trainingStressScore} TSS`
      : '--';

  const handleToggleSelectedInterval = (intervalKey: string) => {
    setSelectedIntervalKey((current) => current === intervalKey ? null : intervalKey);
  };

  return (
    <div className="space-y-6">
      <WorkoutBars bars={bars} />
      {powerSeries.length ? (
        <PowerChart
          activeInterval={activeInterval}
          activeIntervalKey={highlightedIntervalKey}
          intervals={chartIntervalOverlays}
          onHoverIntervalChange={setHoveredIntervalKey}
          onSelectIntervalChange={setSelectedIntervalKey}
          selectedIntervalKey={selectedIntervalKey}
          title={t('calendar.powerChart')}
          values={powerSeries}
        />
      ) : null}
      <div className="grid gap-4 md:grid-cols-4">
        <MetricCard label={t('calendar.duration')} value={formatDurationLabel(durationSeconds)} />
        <MetricCard label="NP" value={normalizedPowerLabel} />
        <MetricCard label="TSS" value={trainingStressLabel} />
        <MetricCard label={t('calendar.compliance')} value={compliance ?? '--'} />
      </div>
      <MatchedIntervalsSection
        highlightedIntervalKey={highlightedIntervalKey}
        intervalRowRefs={intervalRowRefs.current}
        intervals={actualWorkout?.matchedIntervals ?? []}
        onHoverIntervalChange={setHoveredIntervalKey}
        onToggleSelectedInterval={handleToggleSelectedInterval}
        totalDurationSeconds={matchedIntervalTotalDurationSeconds}
      />
      <CompletedIntervalsSection
        activity={activity}
        highlightedIntervalKey={highlightedIntervalKey}
        intervalRowRefs={intervalRowRefs.current}
        intervals={completedIntervals}
        onHoverIntervalChange={setHoveredIntervalKey}
        onToggleSelectedInterval={handleToggleSelectedInterval}
        totalDurationSeconds={completedIntervalTotalDurationSeconds}
      />
      {detailsUnavailableMessage ? (
        <div className="rounded-2xl border border-amber-300/20 bg-amber-300/10 p-4 text-sm text-amber-100">
          {detailsUnavailableMessage ?? t('calendar.importedWorkoutDetailsUnavailable')}
        </div>
      ) : null}
    </div>
  );
}
