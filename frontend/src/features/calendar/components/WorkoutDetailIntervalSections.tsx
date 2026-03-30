import {type KeyboardEvent, type ReactNode} from 'react';
import {useTranslation} from 'react-i18next';

import type {IntervalActivity, IntervalEvent} from '../../intervals/types';
import {formatDurationLabel} from '../workoutDetails';
import type {ChartIntervalOverlay} from './WorkoutDetailPowerChart';

type CompletedInterval = IntervalActivity['details']['intervals'][number];
type MatchedInterval = NonNullable<IntervalEvent['actualWorkout']>['matchedIntervals'][number];

type SharedIntervalSectionProps = {
  highlightedIntervalKey: string | null;
  intervalRowRefs: Map<string, HTMLButtonElement>;
  onHoverIntervalChange: (intervalKey: string | null) => void;
  onToggleSelectedInterval: (intervalKey: string) => void;
};

type MatchedIntervalsSectionProps = SharedIntervalSectionProps & {
  intervals: NonNullable<IntervalEvent['actualWorkout']>['matchedIntervals'];
  totalDurationSeconds: number;
};

type CompletedIntervalsSectionProps = SharedIntervalSectionProps & {
  activity: IntervalActivity | null;
  intervals: CompletedInterval[];
  totalDurationSeconds: number;
};

export function MatchedIntervalsSection({
  highlightedIntervalKey,
  intervalRowRefs,
  intervals,
  onHoverIntervalChange,
  onToggleSelectedInterval,
  totalDurationSeconds,
}: MatchedIntervalsSectionProps) {
  const {t} = useTranslation();

  if (intervals.length === 0) {
    return null;
  }

  return (
    <div className="rounded-2xl border border-white/6 bg-[#171a1d] p-4">
      <p className="text-[10px] font-black uppercase tracking-[0.24em] text-slate-500">{t('calendar.intervalMatches')}</p>
      <div className="mt-4 space-y-3">
        {intervals.map((interval) => {
          const intervalKey = `${interval.plannedSegmentOrder}-${interval.actualIntervalId ?? 'planned'}`;

          return (
            <IntervalRow
              key={intervalKey}
              durationFillPercent={durationFillPercent(matchedIntervalDurationSeconds(interval), totalDurationSeconds)}
              isHighlighted={highlightedIntervalKey === intervalKey}
              onHover={() => onHoverIntervalChange(intervalKey)}
              onHoverEnd={() => onHoverIntervalChange(null)}
              onSelect={() => onToggleSelectedInterval(intervalKey)}
              rowRef={(node) => updateIntervalRowRef(intervalRowRefs, intervalKey, node)}
              left={(
                <div>
                  <p className="text-sm font-bold text-[#f9f9fd]">{interval.plannedLabel}</p>
                  <p className="text-xs text-slate-400">{formatDurationLabel(interval.plannedDurationSeconds)}</p>
                </div>
              )}
              right={(
                <p className="text-xs font-bold uppercase tracking-[0.2em] text-[#d2ff9a]">
                  {Math.round(interval.complianceScore * 100)}% {t('calendar.compliance')}
                </p>
              )}
            />
          );
        })}
      </div>
    </div>
  );
}

export function CompletedIntervalsSection({
  activity,
  highlightedIntervalKey,
  intervalRowRefs,
  intervals,
  onHoverIntervalChange,
  onToggleSelectedInterval,
  totalDurationSeconds,
}: CompletedIntervalsSectionProps) {
  const {t} = useTranslation();

  if (intervals.length === 0) {
    return null;
  }

  return (
    <div className="rounded-2xl border border-white/6 bg-[#171a1d] p-4">
      <p className="text-[10px] font-black uppercase tracking-[0.24em] text-slate-500">{t('calendar.completedIntervals')}</p>
      <div className="mt-4 space-y-3">
        {intervals.map((interval, index) => {
          const intervalKey = `${interval.id ?? 'interval'}-${index}`;

          return (
            <IntervalRow
              key={intervalKey}
              durationFillPercent={durationFillPercent(completedIntervalDurationSeconds(interval), totalDurationSeconds)}
              isHighlighted={highlightedIntervalKey === intervalKey}
              onHover={() => onHoverIntervalChange(intervalKey)}
              onHoverEnd={() => onHoverIntervalChange(null)}
              onSelect={() => onToggleSelectedInterval(intervalKey)}
              rowRef={(node) => updateIntervalRowRef(intervalRowRefs, intervalKey, node)}
              left={(
                <div>
                  <p className="text-sm font-bold text-[#f9f9fd]">{interval.label ?? `${activity?.activityType ?? t('calendar.workout')} ${index + 1}`}</p>
                  <p className="text-xs text-slate-400">{formatDurationLabel(interval.movingTimeSeconds ?? interval.elapsedTimeSeconds ?? null)}</p>
                </div>
              )}
              right={(
                <div className="text-right">
                  <p className="text-xs font-bold uppercase tracking-[0.2em] text-[#d2ff9a]">
                    {interval.averagePowerWatts !== null ? `${interval.averagePowerWatts} W` : '--'}
                  </p>
                  <p className="text-xs text-slate-400">
                    {interval.averageHeartRateBpm !== null ? `${interval.averageHeartRateBpm} bpm` : '--'}
                  </p>
                </div>
              )}
            />
          );
        })}
      </div>
    </div>
  );
}

export function getDisplayableCompletedIntervals(activity: IntervalActivity | null): CompletedInterval[] {
  return activity?.details.intervals.filter(isDisplayableCompletedInterval) ?? [];
}

export function buildChartIntervals(
  event: IntervalEvent | null,
  actualWorkout: IntervalEvent['actualWorkout'],
  activity: IntervalActivity | null,
): ChartIntervalOverlay[] {
  if (actualWorkout?.matchedIntervals.length) {
    return actualWorkout.matchedIntervals
      .filter((interval) => interval.actualStartTimeSeconds !== null || interval.actualEndTimeSeconds !== null)
      .map((interval, index) => ({
        id: `${interval.plannedSegmentOrder}-${interval.actualIntervalId ?? 'planned'}`,
        startSecond: interval.actualStartTimeSeconds ?? runningStart(actualWorkout.matchedIntervals, index),
        endSecond: interval.actualEndTimeSeconds ?? ((interval.actualStartTimeSeconds ?? runningStart(actualWorkout.matchedIntervals, index)) + interval.plannedDurationSeconds),
        label: interval.plannedLabel,
      }));
  }

  if (actualWorkout && event?.eventDefinition.segments.length) {
    return event.eventDefinition.segments.map((segment) => ({
      id: `${segment.order}-planned`,
      startSecond: segment.startOffsetSeconds,
      endSecond: segment.endOffsetSeconds,
      label: segment.label,
    }));
  }

  if (!activity) {
    return [];
  }

  return buildCompletedChartIntervals(activity);
}

export function firstPositiveValue(...values: Array<number | null | undefined>): number {
  for (const value of values) {
    if (value !== null && value !== undefined && value > 0) {
      return value;
    }
  }

  return 0;
}

export function completedIntervalsTotalDuration(intervals: CompletedInterval[], fallbackDurationSeconds: number): number {
  const inferredTotal = intervals.reduce((sum, interval) => sum + completedIntervalDurationSeconds(interval), 0);
  return Math.max(fallbackDurationSeconds, inferredTotal, 1);
}

export function matchedIntervalsTotalDuration(
  intervals: NonNullable<IntervalEvent['actualWorkout']>['matchedIntervals'],
  fallbackDurationSeconds: number,
): number {
  const inferredTotal = intervals.reduce((sum, interval) => sum + matchedIntervalDurationSeconds(interval), 0);
  return Math.max(fallbackDurationSeconds, inferredTotal, 1);
}

function IntervalRow({
  durationFillPercent,
  isHighlighted,
  left,
  onHover,
  onHoverEnd,
  onSelect,
  right,
  rowRef,
}: {
  durationFillPercent: number;
  isHighlighted: boolean;
  left: ReactNode;
  onHover: () => void;
  onHoverEnd: () => void;
  onSelect: () => void;
  right: ReactNode;
  rowRef: (node: HTMLButtonElement | null) => void;
}) {
  const handleKeyDown = (event: KeyboardEvent<HTMLButtonElement>) => {
    if (event.key === 'Enter' || event.key === ' ') {
      event.preventDefault();
      onSelect();
    }
  };

  return (
    <button
      ref={rowRef}
      type="button"
      onClick={onSelect}
      onFocus={onHover}
      onMouseEnter={onHover}
      onMouseLeave={onHoverEnd}
      onBlur={onHoverEnd}
      onKeyDown={handleKeyDown}
      data-interval-row-active={isHighlighted ? 'true' : 'false'}
      className={`relative block w-full overflow-hidden rounded-xl px-4 py-3 text-left transition focus:outline-none focus-visible:ring-2 focus-visible:ring-[#d2ff9a]/40 ${isHighlighted ? 'bg-[#20291d] ring-1 ring-[#d2ff9a]/30' : 'bg-white/[0.03]'}`}
    >
      <div
        data-interval-duration-fill="true"
        className={`absolute inset-y-0 left-0 rounded-r-xl ${isHighlighted ? 'bg-[linear-gradient(90deg,rgba(210,255,154,0.24)_0%,rgba(210,255,154,0.08)_100%)]' : 'bg-[linear-gradient(90deg,rgba(210,255,154,0.14)_0%,rgba(210,255,154,0.04)_100%)]'}`}
        style={{width: `${durationFillPercent}%`}}
      />
      <div className="relative flex items-center justify-between gap-4">
        {left}
        {right}
      </div>
    </button>
  );
}

function isDisplayableCompletedInterval(interval: CompletedInterval): boolean {
  return interval.label !== null
    || interval.movingTimeSeconds !== null
    || interval.elapsedTimeSeconds !== null
    || interval.averagePowerWatts !== null
    || interval.averageHeartRateBpm !== null;
}

function completedIntervalDurationSeconds(interval: CompletedInterval): number {
  const timedDuration = interval.startTimeSeconds !== null && interval.endTimeSeconds !== null
    ? interval.endTimeSeconds - interval.startTimeSeconds
    : interval.movingTimeSeconds ?? interval.elapsedTimeSeconds;

  return Math.max(1, timedDuration ?? 1);
}

function matchedIntervalDurationSeconds(interval: MatchedInterval): number {
  const timedDuration = interval.actualStartTimeSeconds !== null && interval.actualEndTimeSeconds !== null
    ? interval.actualEndTimeSeconds - interval.actualStartTimeSeconds
    : interval.plannedDurationSeconds;

  return Math.max(1, timedDuration);
}

function durationFillPercent(durationSeconds: number, totalDurationSeconds: number): number {
  return Math.max(4, Math.min(100, (durationSeconds / Math.max(1, totalDurationSeconds)) * 100));
}

function runningStart<T extends {
  actualStartTimeSeconds?: number | null;
  actualEndTimeSeconds?: number | null;
  plannedDurationSeconds?: number;
  startTimeSeconds?: number | null;
  endTimeSeconds?: number | null;
  movingTimeSeconds?: number | null;
  elapsedTimeSeconds?: number | null;
}>(intervals: T[], index: number): number {
  return intervals.slice(0, index).reduce((sum, interval) => {
    if (interval.actualEndTimeSeconds !== null && interval.actualEndTimeSeconds !== undefined) {
      return interval.actualEndTimeSeconds;
    }
    if (interval.endTimeSeconds !== null && interval.endTimeSeconds !== undefined) {
      return interval.endTimeSeconds;
    }

    const duration = interval.plannedDurationSeconds
      ?? interval.movingTimeSeconds
      ?? interval.elapsedTimeSeconds
      ?? ((interval.actualEndTimeSeconds ?? interval.endTimeSeconds ?? 0) - (interval.actualStartTimeSeconds ?? interval.startTimeSeconds ?? 0));

    return sum + Math.max(0, duration);
  }, 0);
}

function buildCompletedChartIntervals(activity: IntervalActivity): ChartIntervalOverlay[] {
  const overlays: ChartIntervalOverlay[] = [];
  let cursor = 0;

  activity.details.intervals.forEach((interval, index) => {
    const startSecond = interval.startTimeSeconds ?? cursor;
    const durationSeconds = completedIntervalDurationSeconds(interval);
    const endSecond = interval.endTimeSeconds ?? (startSecond + durationSeconds);

    if (isDisplayableCompletedInterval(interval) && hasChartTiming(interval)) {
      overlays.push({
        id: `${interval.id ?? 'interval'}-${index}`,
        startSecond,
        endSecond,
        label: interval.label ?? `${activity.activityType ?? 'Workout'} ${index + 1}`,
      });
    }

    cursor = Math.max(cursor, endSecond);
  });

  return overlays;
}

function hasChartTiming(interval: CompletedInterval): boolean {
  return interval.startTimeSeconds !== null
    || interval.endTimeSeconds !== null
    || interval.movingTimeSeconds !== null
    || interval.elapsedTimeSeconds !== null;
}

function updateIntervalRowRef(refs: Map<string, HTMLButtonElement>, key: string, node: HTMLButtonElement | null) {
  if (node) {
    refs.set(key, node);
  } else {
    refs.delete(key);
  }
}
