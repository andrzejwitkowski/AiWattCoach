import type { IntervalActivity, IntervalEvent } from '../intervals/types';

import type { WorkoutBar } from './workoutDetails';

const BAR_HEIGHT_FLOOR = 2;
const BAR_HEIGHT_CEIL = 100;

export function normalizeSkylineWidths(widths: number[], barCount: number): number[] {
  if (barCount === 0) {
    return [];
  }

  const rawWidths = Array.from({length: barCount}, (_, index) => widths[index] ?? 1);
  const maxWidth = Math.max(...rawWidths, 1);

  return rawWidths.map((width) => {
    const normalized = maxWidth > 512 ? Math.round(width / 109) : width;
    return Math.max(1, normalized);
  });
}

export function completedIntervalDurationSeconds(interval: IntervalActivity['details']['intervals'][number]): number {
  const inferredDuration = interval.startTimeSeconds !== null && interval.endTimeSeconds !== null
    ? interval.endTimeSeconds - interval.startTimeSeconds
    : null;

  return normalizeWidthUnits(interval.movingTimeSeconds ?? interval.elapsedTimeSeconds ?? inferredDuration);
}

export function matchedIntervalDurationSeconds(interval: NonNullable<IntervalEvent['actualWorkout']>['matchedIntervals'][number]): number {
  const inferredDuration = interval.actualStartTimeSeconds !== null && interval.actualEndTimeSeconds !== null
    ? interval.actualEndTimeSeconds - interval.actualStartTimeSeconds
    : null;

  return normalizeWidthUnits(inferredDuration ?? interval.plannedDurationSeconds);
}

export function heightForPercent(percent: number | null | undefined): number {
  if (!percent || percent <= 0) {
    return 4;
  }

  return Math.max(2, Math.min(100, Math.round(percent)));
}

export function heightForPower(power: number): number {
  if (!Number.isFinite(power) || power <= 0) {
    return 4;
  }

  return Math.max(2, Math.min(100, Math.round(power / 13)));
}

export function normalizeBarHeights(bars: WorkoutBar[]): WorkoutBar[] {
  if (bars.length <= 1) {
    return bars;
  }

  const heights = bars.map((bar) => bar.height);
  const min = Math.min(...heights);
  const max = Math.max(...heights);
  if (max <= min) {
    return bars;
  }

  const scale = (BAR_HEIGHT_CEIL - BAR_HEIGHT_FLOOR) / (max - min);

  return bars.map((bar) => {
    const normalizedHeight = BAR_HEIGHT_FLOOR + (bar.height - min) * scale;

    return {
      ...bar,
      height: Math.max(BAR_HEIGHT_FLOOR, Math.min(BAR_HEIGHT_CEIL, Math.round(normalizedHeight * 100) / 100)),
    };
  });
}

function normalizeWidthUnits(durationSeconds: number | null | undefined): number {
  if (!durationSeconds || durationSeconds <= 0) {
    return 1;
  }

  return durationSeconds;
}
